use rust_decimal::Decimal;

#[derive(Debug)]
pub struct Account {
    pub available: Decimal, // funds available for withdrawal
    pub held: Decimal,      // funds held due to disputes
    pub total: Decimal,     // total funds = available + held
    pub locked: bool,       // account frozen due to chargeback
}

impl Account {
    pub fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn sync_total(&mut self) {
        self.total = self.available + self.held;
    }
}
