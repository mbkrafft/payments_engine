use crate::domain::Money;

pub struct Account {
    available: Money, // funds available for withdrawal
    held: Money,      // funds held due to disputes
    total: Money,     // total funds = available + held
    locked: bool,     // account frozen due to chargeback
}
