use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::NamedTempFile;

fn cli_command() -> Command {
    Command::cargo_bin("cli").unwrap()
}

#[test]
fn create_prints_ok() {
    let db = NamedTempFile::new().unwrap();
    cli_command()
        .arg(db.path())
        .arg(r#"create({"_id":"u1","name":"Anna"})"#)
        .assert()
        .success()
        .stdout("ok\n");
}

#[test]
fn get_prints_json() {
    let db = NamedTempFile::new().unwrap();

    cli_command()
        .arg(db.path())
        .arg(r#"create({"_id":"u1","name":"Anna"})"#)
        .assert()
        .success();

    let output = cli_command()
        .arg(db.path())
        .arg(r#"get({"_id":"u1"})"#)
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["name"], "Anna");
}

#[test]
fn delete_prints_boolean() {
    let db = NamedTempFile::new().unwrap();

    cli_command()
        .arg(db.path())
        .arg(r#"create({"_id":"u1","name":"Anna"})"#)
        .assert()
        .success();

    cli_command()
        .arg(db.path())
        .arg(r#"delete({"_id":"u1"})"#)
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn dump_prints_json_array() {
    let db = NamedTempFile::new().unwrap();

    cli_command()
        .arg(db.path())
        .arg(r#"create({"_id":"u1","name":"Anna"})"#)
        .assert()
        .success();

    let output = cli_command().arg(db.path()).arg("dump()").output().unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 1);
}

#[test]
fn invalid_command_exits_nonzero() {
    let db = NamedTempFile::new().unwrap();

    let output = cli_command().arg(db.path()).arg("wat()").output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown command"));
}
