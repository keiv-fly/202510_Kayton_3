use assert_cmd::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn run_command_executes_program() {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(
        file,
        "fn add(a, b):\n    a + b\n\nfn main():\n    add(2, 3)\n"
    )
    .expect("write source");

    let mut cmd = Command::cargo_bin("kayton-cli").expect("binary");
    cmd.arg("run")
        .arg(file.path())
        .assert()
        .success()
        .stdout("5\n");
}
