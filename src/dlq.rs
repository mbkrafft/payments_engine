use crate::domain::{DeadLetterQueue, Error};

#[derive(Default, Debug)]
pub struct StdErrDLQ {}

impl DeadLetterQueue for StdErrDLQ {
    fn report(&self, error: &Error) {
        eprintln!("DLQ Report - Error: {}", error);
    }
}
