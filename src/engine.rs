use crate::domain::{
    Error, Transaction, TransactionKind,
    traits::{DeadLetterQueue, OutputRepository, TransactionStream},
};

use futures::StreamExt;

#[derive(Debug)]
pub struct Engine<I, O, D>
where
    I: TransactionStream,
    O: OutputRepository,
    D: DeadLetterQueue,
{
    ingestion: I,
    output_repository: O,
    dlq: D,
}

impl<I, O, D> Engine<I, O, D>
where
    I: TransactionStream,
    O: OutputRepository,
    D: DeadLetterQueue,
{
    pub fn new(ingestion: I, output_repository: O, dlq: D) -> Self {
        Self {
            ingestion,
            output_repository,
            dlq,
        }
    }

    pub async fn process(&mut self) -> Result<(), Error> {
        let mut res = self.ingestion.stream();

        while let Some(tx) = res.next().await {
            match tx {
                Ok(tx) => match self.apply_transaction(tx) {
                    Ok(()) => {}
                    Err(e) => self.dlq.report(&e),
                },
                Err(e) => self.dlq.report(&e),
            }
        }

        Ok(())
    }

    fn apply_transaction(&mut self, tx: Transaction) -> Result<(), Error> {
        {
            let account = self.output_repository.get_or_create_account(&tx.client_id);

            if account.locked {
                return Err(Error::Engine(
                    tx.client_id.to_string() + " account is locked",
                ));
            }
        }

        match tx.kind {
            TransactionKind::Deposit { amount } => self.deposit(&tx, amount),
            TransactionKind::Withdrawal { amount } => self.withraw(&tx, amount),
            TransactionKind::Dispute => self.dispute(&tx),
            TransactionKind::Resolve => self.resolve(&tx),
            TransactionKind::Chargeback => self.chargeback(tx),
        }
    }

    fn deposit(&mut self, tx: &Transaction, amount: rust_decimal::Decimal) -> Result<(), Error> {
        match self
            .output_repository
            .report_transaction(&tx.transaction_id, tx)
        {
            Ok(_) => {
                let account = self.output_repository.get_or_create_account(&tx.client_id);
                account.available += amount;
                account.sync_total();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn withraw(&mut self, tx: &Transaction, amount: rust_decimal::Decimal) -> Result<(), Error> {
        match self
            .output_repository
            .report_transaction(&tx.transaction_id, tx)
        {
            Ok(_) => {
                let account = self.output_repository.get_or_create_account(&tx.client_id);

                if account.available < amount {
                    return Err(Error::Engine(
                        format!("Insufficient funds for client {}", tx.client_id).to_owned(),
                    ));
                }

                account.available -= amount;
                account.sync_total();

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn dispute(&mut self, tx: &Transaction) -> Result<(), Error> {
        let disputed_tx = self
            .output_repository
            .get_transaction(tx.transaction_id)
            .ok_or_else(|| Error::Engine("Referenced transaction not found".to_string()))?;

        if disputed_tx.client_id != tx.client_id {
            return Err(Error::Engine(
                "Transaction client ID does not match dispute client ID".to_string(),
            ));
        }

        if let TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } =
            disputed_tx.kind
        {
            {
                self.output_repository
                    .mark_transaction_disputed(tx.transaction_id);
            }
            let account = self.output_repository.get_or_create_account(&tx.client_id);
            account.available -= amount;
            account.held += amount;
        }

        Ok(())
    }

    fn resolve(&mut self, tx: &Transaction) -> Result<(), Error> {
        {
            if !self.output_repository.has_dispute(tx.transaction_id) {
                return Err(Error::Engine("Transaction is not disputed".to_string()));
            }
        }

        let resolved_tx = self
            .output_repository
            .get_transaction(tx.transaction_id)
            .ok_or_else(|| Error::Engine("Referenced transaction not found".to_string()))?;

        if resolved_tx.client_id != tx.client_id {
            return Err(Error::Engine(
                "Transaction client ID does not match resolve client ID".to_string(),
            ));
        }

        if let TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } =
            resolved_tx.kind
        {
            {
                self.output_repository
                    .mark_transaction_resolved(tx.transaction_id);
            }

            let account = self.output_repository.get_or_create_account(&tx.client_id);
            account.available += amount;
            account.held -= amount;
        }
        Ok(())
    }

    fn chargeback(&mut self, tx: Transaction) -> Result<(), Error> {
        {
            if !self.output_repository.has_dispute(tx.transaction_id) {
                return Err(Error::Engine("Transaction is not disputed".to_string()));
            }
        }

        let chargeback_tx = self
            .output_repository
            .get_transaction(tx.transaction_id)
            .ok_or_else(|| Error::Engine("Referenced transaction not found".to_string()))?;

        if chargeback_tx.client_id != tx.client_id {
            return Err(Error::Engine(
                "Transaction client ID does not match resolve client ID".to_string(),
            ));
        }

        if let TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } =
            chargeback_tx.kind
        {
            // (Only if orig_tx was under dispute)
            let account = self.output_repository.get_or_create_account(&tx.client_id);
            account.available += amount;
            account.held -= amount;
            account.locked = true;
        }
        Ok(())
    }

    pub fn flush(&mut self) {
        self.output_repository.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output_repository::StdOutOutput;
    use futures::stream::{self, Stream};
    use rust_decimal::Decimal;
    use std::pin::Pin;

    #[derive(Debug, Default)]
    struct NoopIngestion;

    impl TransactionStream for NoopIngestion {
        type TxStream = Pin<Box<dyn Stream<Item = Result<Transaction, Error>> + Send>>;
        fn stream(&mut self) -> Self::TxStream {
            Box::pin(stream::iter(Vec::<Result<Transaction, Error>>::new()))
        }
    }

    #[derive(Default, Debug)]
    struct NoopDLQ;

    impl DeadLetterQueue for NoopDLQ {
        fn report(&self, _error: &Error) {}
    }

    fn mk_engine() -> Engine<NoopIngestion, StdOutOutput, NoopDLQ> {
        Engine::new(NoopIngestion, StdOutOutput::new(), NoopDLQ)
    }

    #[test]
    fn deposit_increases_available_and_total() {
        let mut engine = mk_engine();
        let tx = Transaction {
            kind: TransactionKind::Deposit {
                amount: Decimal::from(100u32),
            },
            client_id: 1,
            transaction_id: 1,
        };

        engine
            .deposit(&tx, Decimal::from(100u32))
            .expect("deposit ok");

        let acct = engine.output_repository.get_or_create_account(&1);
        assert_eq!(acct.available, Decimal::from(100u32));
        assert_eq!(acct.held, Decimal::from(0u32));
        assert_eq!(acct.total, Decimal::from(100u32));
        assert!(!acct.locked);
    }

    #[test]
    fn withdrawal_with_insufficient_funds_errors_and_does_not_change_balances() {
        let mut engine = mk_engine();
        let tx = Transaction {
            kind: TransactionKind::Withdrawal {
                amount: Decimal::from(50u32),
            },
            client_id: 1,
            transaction_id: 2,
        };

        let res = engine.withraw(&tx, Decimal::from(50u32));
        assert!(res.is_err());

        let acct = engine.output_repository.get_or_create_account(&1);
        assert_eq!(acct.available, Decimal::from(0u32));
        assert_eq!(acct.held, Decimal::from(0u32));
        assert_eq!(acct.total, Decimal::from(0u32));
    }

    #[test]
    fn dispute_moves_available_to_held_and_marks_disputed() {
        let mut engine = mk_engine();
        let dep = Transaction {
            kind: TransactionKind::Deposit {
                amount: Decimal::from(75u32),
            },
            client_id: 1,
            transaction_id: 10,
        };
        engine.deposit(&dep, Decimal::from(75u32)).unwrap();

        let dispute = Transaction {
            kind: TransactionKind::Dispute,
            client_id: 1,
            transaction_id: 10,
        };
        engine.dispute(&dispute).expect("dispute ok");

        let acct = engine.output_repository.get_or_create_account(&1);
        assert_eq!(acct.available, Decimal::from(0u32));
        assert_eq!(acct.held, Decimal::from(75u32));
        assert_eq!(acct.available + acct.held, acct.total);
        assert!(engine.output_repository.has_dispute(10));
    }

    #[test]
    fn resolve_moves_held_back_and_clears_dispute() {
        let mut engine = mk_engine();
        let dep = Transaction {
            kind: TransactionKind::Deposit {
                amount: Decimal::from(40u32),
            },
            client_id: 2,
            transaction_id: 20,
        };
        engine.deposit(&dep, Decimal::from(40u32)).unwrap();
        let dispute = Transaction {
            kind: TransactionKind::Dispute,
            client_id: 2,
            transaction_id: 20,
        };
        engine.dispute(&dispute).unwrap();

        let resolve = Transaction {
            kind: TransactionKind::Resolve,
            client_id: 2,
            transaction_id: 20,
        };
        engine.resolve(&resolve).expect("resolve ok");

        let acct = engine.output_repository.get_or_create_account(&2);
        assert_eq!(acct.available, Decimal::from(40u32));
        assert_eq!(acct.held, Decimal::from(0u32));
        assert_eq!(acct.available + acct.held, acct.total);
        assert!(!engine.output_repository.has_dispute(20));
    }

    #[test]
    fn chargeback_locks_account_and_adjusts_balances() {
        let mut engine = mk_engine();
        let dep = Transaction {
            kind: TransactionKind::Deposit {
                amount: Decimal::from(60u32),
            },
            client_id: 3,
            transaction_id: 30,
        };
        engine.deposit(&dep, Decimal::from(60u32)).unwrap();
        let dispute = Transaction {
            kind: TransactionKind::Dispute,
            client_id: 3,
            transaction_id: 30,
        };
        engine.dispute(&dispute).unwrap();

        // Perform chargeback
        let chargeback = Transaction {
            kind: TransactionKind::Chargeback,
            client_id: 3,
            transaction_id: 30,
        };
        engine.chargeback(chargeback).expect("chargeback ok");

        let acct = engine.output_repository.get_or_create_account(&3);
        assert!(acct.locked);
        assert_eq!(acct.available, Decimal::from(60u32));
        assert_eq!(acct.held, Decimal::from(0u32));
        assert_eq!(acct.total, Decimal::from(60u32));
    }
}
