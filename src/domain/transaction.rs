use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy)]
pub enum TransactionKind {
    Deposit { amount: Decimal },
    Withdrawal { amount: Decimal },
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub kind: TransactionKind,
    pub client_id: u16,
    pub transaction_id: u32,
}

impl core::fmt::Display for Transaction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } => {
                write!(
                    f,
                    "{:?},client={},tx={},amount={}",
                    self.kind, self.client_id, self.transaction_id, amount
                )
            }
            _ => write!(
                f,
                "{:?},client={},tx={}",
                self.kind, self.client_id, self.transaction_id
            ),
        }
    }
}
