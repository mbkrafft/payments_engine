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
deposit, 1, 1, 1.0001\n\
deposit, 1, 2, 0.5003\n\
deposit, 2, 3, 2.0003"
    )
    .unwrap();

    let exe = env!("CARGO_BIN_EXE_payments_engine");
    let mut cmd = Command::new(exe);
    cmd.arg(file.path());

    cmd.assert()
        .success()
        .stdout(pred::str::contains("client,available,held,total,locked"))
        .stdout(pred::str::contains("1,1.5004,0,1.5004,false"))
        .stdout(pred::str::contains("2,2.0003,0,2.0003,false"));
}
