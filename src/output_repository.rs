use std::collections::HashMap;

use crate::domain::{Account, Error, OutputRepository, Transaction};
use std::collections::hash_map::Entry;

#[derive(Default, Debug)]
pub struct StdOutOutput {
    accounts: HashMap<u16, Account>,
    ledger: HashMap<u32, Transaction>,
}

impl StdOutOutput {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            ledger: HashMap::new(),
        }
    }
}

impl OutputRepository for StdOutOutput {
    fn get_or_create_account(&mut self, client_id: &u16) -> &mut Account {
        self.accounts.entry(*client_id).or_insert_with(Account::new)
    }

    fn report_transaction(
        &mut self,
        transaction_id: &u32,
        transaction: &Transaction,
    ) -> Result<(), Error> {
        match self.ledger.entry(*transaction_id) {
            Entry::Vacant(e) => {
                e.insert(transaction.clone());
                Ok(())
            }
            Entry::Occupied(_) => Err(Error::Engine(format!(
                "Transaction ID {} already exists",
                transaction_id
            ))),
        }
    }

    fn get_transaction(&mut self, transaction_id: u32) -> Option<&Transaction> {
        self.ledger.get(&transaction_id)
    }

    fn flush(&mut self) {
        println!("client,available,held,total,locked");
        for (client_id, account) in &self.accounts {
            println!(
                "{},{},{},{},{}",
                client_id,
                account.available.round_dp(4),
                account.held.round_dp(4),
                account.total.round_dp(4),
                account.locked
            );
        }
    }
}
