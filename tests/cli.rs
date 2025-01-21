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

use tempfile::NamedTempFile;

fn run_pegviz(input_file: &'static str) -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let temp_path = temp_file.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("pegviz")?;

    cmd.arg(path_to_test_resource(input_file))
        .arg("--output")
        .arg(temp_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("pegviz generated to"));

    Ok(())
}

#[test]
fn main_when_valid_character_ranges_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    run_pegviz("ranges.txt")
}

#[test]
fn main_when_valid_token_indices_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    run_pegviz("indices.txt")
}
