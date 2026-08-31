use std::process::Command;

const BINARY: &str = env!("CARGO_BIN_EXE_guide-agent");
const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn run(arguments: &[&str]) -> (bool, String) {
    let output = Command::new(BINARY)
        .args(arguments)
        .env_remove("OPENAI_API_KEY")
        .output()
        .expect("guide-agent binary runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
    )
}

#[test]
fn missing_arguments_print_usage_error() {
    let (success, stdout) = run(&[]);
    assert!(!success);
    assert!(stdout.to_lowercase().contains("usage"));
}

#[test]
fn unsupported_provider_fails_clearly() {
    let (success, stdout) = run(&[
        "ask",
        "hello",
        "--data",
        DATA_DIRECTORY,
        "--provider",
        "carrier-pigeon",
        "--model",
        "test",
    ]);
    assert!(!success);
    assert!(stdout.contains("unsupported provider"));
}

#[test]
fn openai_provider_requires_environment_key() {
    let (success, stdout) = run(&[
        "ask",
        "hello",
        "--data",
        DATA_DIRECTORY,
        "--provider",
        "openai",
        "--model",
        "test",
    ]);
    assert!(!success);
    assert!(stdout.contains("OPENAI_API_KEY"));
}
