use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{path::PathBuf, process::Command};

pub fn path_to_test_resource(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("resources");
    path.push(name);
    path
}

#[test]
fn main_when_valid_character_ranges_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("pegviz")?;

    cmd.arg(path_to_test_resource("ranges.txt"))
        .arg("--output")
        .arg("output.html");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("pegviz generated to"));

    Ok(())
}

#[test]
fn main_when_valid_token_indices_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("pegviz")?;

    cmd.arg(path_to_test_resource("indices.txt"))
        .arg("--output") 
        .arg("output.html");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("pegviz generated to"));

    Ok(())
}