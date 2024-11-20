#![cfg(test)]

use insta::with_settings;
use itertools::Itertools;
use regex::Regex;
use std::env;
use std::path::PathBuf;
use std::process::Command;

macro_rules! run_acceptance_test {
    ($cmd:expr, $case_name:expr) => {
        with_settings!({snapshot_path => get_snapshot_directory()}, {
            let subcommand = Command::new("cargo")
                .current_dir(PathBuf::from("tests").join("acceptance_cases").join($case_name))
                .args(&[$cmd])
                .output()
                .expect("Failed to spawn cargo subcommand");

            let mut output = String::new();
            output.push_str(String::from_utf8_lossy(&subcommand.stdout).as_ref());
            output.push_str(String::from_utf8_lossy(&subcommand.stderr).as_ref());

            let output = sanitize_lines(output);

            insta::assert_snapshot!(output);
        })
    };
    ($case_name:expr) => {
        run_acceptance_test!("test", $case_name)
    }
}

fn get_snapshot_directory() -> String {
    PathBuf::from("snapshots")
        .join(env::var("SNAPSHOT_DIR").unwrap_or_else(|_| "rust-stable".to_string()))
        .to_str()
        .unwrap()
        .to_string()
}

fn sanitize_lines(s: String) -> String {
    let re_time = Regex::new(r"\d+\.\d{2}s").expect("Building regex");

    let mut s = s
        .lines()
        .filter(|line| {
            (line.starts_with("test") // For general test output
                || line.contains("panicked at") // For panic messages
                || line.starts_with("error:") // For validating red paths
                || line.starts_with("error[")) // For validating red paths
                && !line.contains("process didn't exit successfully") // for Windows
        })
        .map(|line| line.replace('\\', "/"))
        .map(|line| line.replace(".exe", ""))
        .map(|line| re_time.replace_all(&line, "0.00s").to_string())
        .collect::<Vec<_>>();

    s.sort_unstable();

    s.into_iter().join("\n")
}

#[test]
fn cases_can_be_declared_on_async_methods() {
    run_acceptance_test!("cases_can_be_declared_on_async_methods")
}

#[test]
fn cases_can_be_declared_on_non_test_items() {
    run_acceptance_test!("cases_can_be_declared_on_non_test_items")
}

#[test]
fn cases_declared_on_non_test_items_can_be_used() {
    run_acceptance_test!("run", "cases_can_be_declared_on_non_test_items")
}

#[test]
fn cases_can_be_ignored() {
    run_acceptance_test!("cases_can_be_ignored")
}

#[test]
fn cases_can_panic() {
    run_acceptance_test!("cases_can_panic")
}

#[test]
fn cases_can_return_result() {
    run_acceptance_test!("cases_can_return_result")
}

#[test]
fn cases_support_basic_features() {
    run_acceptance_test!("cases_support_basic_features")
}

#[test]
fn matrices_support_basic_features() {
    run_acceptance_test!("matrices_support_basic_features")
}

#[test]
fn cases_support_complex_assertions() {
    run_acceptance_test!("cases_support_complex_assertions")
}

#[test]
fn cases_support_generics() {
    run_acceptance_test!("cases_support_generics")
}

#[test]
fn cases_support_keyword_using() {
    run_acceptance_test!("cases_support_keyword_using")
}

#[test]
fn cases_support_keyword_with() {
    run_acceptance_test!("cases_support_keyword_with")
}

#[test]
fn cases_support_multiple_calling_methods() {
    run_acceptance_test!("cases_support_multiple_calling_methods")
}

#[test]
fn cases_support_pattern_matching() {
    run_acceptance_test!("cases_support_pattern_matching")
}

#[test]
fn cases_can_use_regex() {
    run_acceptance_test!("cases_can_use_regex")
}

#[test]
fn features_produce_human_readable_errors() {
    run_acceptance_test!("features_produce_human_readable_errors")
}

#[test]
fn allow_stays_on_fn() {
    run_acceptance_test!("allow_stays_on_fn")
}

#[test]
fn matrices_compilation_errors() {
    run_acceptance_test!("matrices_compilation_errors")
}
