use std::io::Write;

use assert_cmd::Command;
use predicates as pred;
use tempfile::NamedTempFile;

#[test]
fn end_to_end_outputs_expected_balances() {
    // Prepare a temporary CSV file with transactions that yield
    // client 1: 1.5 available, 0 held, total 1.5, unlocked
    // client 2: 2.0 available, 0 held, total 2.0, unlocked
    let mut file = NamedTempFile::new().expect("create temp file");
    writeln!(
        file,
        "type, client, tx, amount\n\
    deposit, 1, 1, 100.0003\n\
    deposit, 2, 2, 50.0001\n\
    withdrawal, 1, 3, 30.0\n\
    dispute, 1, 1,\n\
    resolve, 1, 1,\n\
    withdrawal, 2, 4, 60.0\n\
    chargeback, 1, 1,\n\
    chah, 1,\n\
    chargeback, 1, 1\n\
    dispute, 2, 2,\n\
    chargeback, 2, 2"
    )
    .unwrap();

    let exe = env!("CARGO_BIN_EXE_payments_engine");
    let mut cmd = Command::new(exe);
    cmd.arg(file.path());

    cmd.assert()
        .success()
        .stdout(pred::str::contains("client,available,held,total,locked"))
        .stdout(pred::str::contains("1,70.0003,0.0000,70.0003,false"))
        .stdout(pred::str::contains("2,50.0001,0.0000,50.0001,true"));
}
