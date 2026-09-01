use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

const BINARY: &str = env!("CARGO_BIN_EXE_guide-server");
const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn run(arguments: &[&str], environment: &[(&str, Option<&str>)]) -> (bool, String) {
    let mut command = Command::new(BINARY);
    command.args(arguments);
    for (name, value) in environment {
        match value {
            Some(value) => {
                command.env(name, value);
            }
            None => {
                command.env_remove(name);
            }
        }
    }
    let output = command.output().expect("guide-server binary runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
    )
}

fn base_environment(provider: Option<&str>) -> Vec<(&'static str, Option<&str>)> {
    vec![
        ("GUIDE_PROVIDER", provider),
        ("GUIDE_MODEL", Some("test-model")),
        ("GUIDE_BASE_URL", None),
        ("OPENAI_API_KEY", None),
    ]
}

#[test]
fn missing_data_path_prints_usage() {
    let (success, stdout) = run(
        &["--port", "8070", "--timeout-seconds", "30"],
        &base_environment(Some("ollama")),
    );
    assert!(!success);
    assert!(stdout.to_lowercase().contains("usage"));
    assert!(stdout.contains("--data"));
}

#[test]
fn host_configuration_is_rejected_because_server_is_loopback_only() {
    let (success, stdout) = run(
        &[
            "--data",
            DATA_DIRECTORY,
            "--host",
            "0.0.0.0",
            "--port",
            "8070",
        ],
        &base_environment(Some("ollama")),
    );
    assert!(!success);
    assert!(stdout.to_lowercase().contains("loopback-only"));
}

#[test]
fn missing_provider_environment_fails_before_binding() {
    let (success, stdout) = run(
        &[
            "--data",
            DATA_DIRECTORY,
            "--port",
            "8070",
            "--timeout-seconds",
            "30",
        ],
        &base_environment(None),
    );
    assert!(!success);
    assert!(stdout.contains("GUIDE_PROVIDER"));
}

#[test]
fn missing_model_environment_fails_before_binding() {
    let environment = vec![
        ("GUIDE_PROVIDER", Some("ollama")),
        ("GUIDE_MODEL", None),
        ("GUIDE_BASE_URL", None),
        ("OPENAI_API_KEY", None),
    ];
    let (success, stdout) = run(
        &[
            "--data",
            DATA_DIRECTORY,
            "--port",
            "8070",
            "--timeout-seconds",
            "30",
        ],
        &environment,
    );
    assert!(!success);
    assert!(stdout.contains("GUIDE_MODEL"));
}

#[test]
fn openai_provider_requires_environment_key_without_echoing_it() {
    let (success, stdout) = run(
        &[
            "--data",
            DATA_DIRECTORY,
            "--port",
            "8070",
            "--timeout-seconds",
            "30",
        ],
        &base_environment(Some("openai")),
    );
    assert!(!success);
    assert!(stdout.contains("OPENAI_API_KEY"));
    assert!(!stdout.contains("test-model"));
}

#[test]
fn ollama_default_and_explicit_base_url_are_accepted() {
    starts_and_requires_manual_shutdown(&[]);
    starts_and_requires_manual_shutdown(&[("GUIDE_BASE_URL", "http://127.0.0.1:11434")]);
}

fn starts_and_requires_manual_shutdown(environment: &[(&str, &str)]) {
    let mut command = Command::new(BINARY);
    command
        .args([
            "--data",
            DATA_DIRECTORY,
            "--port",
            "0",
            "--timeout-seconds",
            "30",
        ])
        .env("GUIDE_PROVIDER", "ollama")
        .env("GUIDE_MODEL", "test-model")
        .env_remove("OPENAI_API_KEY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in environment {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("guide-server starts");
    sleep(Duration::from_millis(300));
    let early_status = child.try_wait().expect("guide-server status reads");
    if let Some(status) = early_status {
        let output = child.wait_with_output().expect("guide-server output reads");
        panic!(
            "guide-server exited early with {status}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    child.kill().expect("guide-server test process stops");
    child.wait().expect("guide-server test process reaps");
}
