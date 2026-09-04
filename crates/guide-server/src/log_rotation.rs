//! Log rotation and secret redaction for the chat debug log.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration for log file rotation.
#[derive(Debug, Clone)]
pub struct LogRotationConfig {
    /// Maximum file size in bytes before rotation occurs.
    pub max_file_bytes: u64,
    /// Maximum number of rotated files to keep.
    pub max_rotated_files: usize,
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 10 * 1024 * 1024, // 10 MB
            max_rotated_files: 5,
        }
    }
}

/// Patterns to redact from log files.
const REDACT_PATTERNS: &[&str] = &[
    "Bearer ",
    "OPENAI_API_KEY=",
    "PALWORLD_GUIDER_GATEWAY_TOKEN=",
    "GUIDE_GATEWAY_TOKEN=",
    "sk-",
];

/// Check if a log file needs rotation and rotate it if necessary.
///
/// If the file at `path` exists and its size exceeds `config.max_file_bytes`,
/// it is renamed to `{path}.rotated-{timestamp_ms}`. Old rotated files beyond
/// `config.max_rotated_files` are deleted. If the file does not exist, this
/// is a no-op.
pub fn rotate_log_if_needed(path: &Path, config: &LogRotationConfig) -> Result<(), String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(()), // File doesn't exist yet; no rotation needed
    };

    if metadata.len() < config.max_file_bytes {
        return Ok(());
    }

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();

    let rotated_path = add_rotation_suffix(path, &timestamp_ms.to_string());

    // Rename the current log to the rotated name
    fs::rename(path, &rotated_path).map_err(|e| format!("failed to rotate log: {e}"))?;

    // Redact sensitive patterns from the rotated file
    redact_file(&rotated_path)?;

    // Clean up old rotated files
    cleanup_old_rotated_files(path, config.max_rotated_files)?;

    Ok(())
}

/// Add a `.rotated-{suffix}` suffix to a path.
fn add_rotation_suffix(path: &Path, suffix: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let rotated_name = format!("{file_name}.rotated-{suffix}");
    parent.join(rotated_name)
}

/// Redact sensitive patterns from a file's content.
fn redact_file(path: &Path) -> Result<(), String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Ok(()), // Can't read; skip redaction
    };

    let mut redacted = content.clone();
    for pattern in REDACT_PATTERNS {
        redacted = redact_pattern(&redacted, pattern);
    }

    if redacted != content {
        fs::write(path, redacted).map_err(|e| format!("failed to write redacted log: {e}"))?;
    }

    Ok(())
}

/// Replace the content after a pattern with `[REDACTED]` on each line.
fn redact_pattern(content: &str, pattern: &str) -> String {
    content
        .lines()
        .map(|line| {
            if let Some(pos) = line.find(pattern) {
                let before = &line[..pos];
                format!("{before}{pattern}[REDACTED]")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Delete old rotated files beyond the maximum count.
fn cleanup_old_rotated_files(path: &Path, max_files: usize) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let base_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut rotated_files: Vec<(PathBuf, u64)> = Vec::new();

    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().into_owned();
            if entry_name.starts_with(&format!("{base_name}.rotated-")) {
                let timestamp_str = entry_name
                    .strip_prefix(&format!("{base_name}.rotated-"))
                    .unwrap_or("0");
                let timestamp: u64 = timestamp_str.parse().unwrap_or(0);
                rotated_files.push((entry.path(), timestamp));
            }
        }
    }

    rotated_files.sort_by_key(|(_, ts)| u64::MAX - ts); // Newest first

    for (file_path, _) in rotated_files.iter().skip(max_files) {
        let _ = fs::remove_file(file_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut file = fs::File::create(path).expect("test file creates");
        file.write_all(content.as_bytes())
            .expect("test file writes");
    }

    #[test]
    fn small_log_is_not_rotated() {
        let temp = std::env::temp_dir();
        let path = temp.join("guide_test_small.log");
        write_test_file(&path, "small content");
        let config = LogRotationConfig {
            max_file_bytes: 100,
            max_rotated_files: 3,
        };
        rotate_log_if_needed(&path, &config).expect("rotation succeeds");
        assert!(path.exists(), "original file still exists");
        assert_eq!(fs::read_to_string(&path).unwrap(), "small content");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn large_log_is_rotated() {
        let temp = std::env::temp_dir();
        let path = temp.join("guide_test_large.log");
        let content = "x".repeat(200);
        write_test_file(&path, &content);
        let config = LogRotationConfig {
            max_file_bytes: 100,
            max_rotated_files: 3,
        };
        rotate_log_if_needed(&path, &config).expect("rotation succeeds");
        assert!(!path.exists(), "original file was renamed");
        let _ = fs::remove_file(&path);
        // Clean up rotated files
        if let Ok(entries) = fs::read_dir(temp) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("guide_test_large.log.rotated-") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    #[test]
    fn missing_log_is_noop() {
        let path = Path::new("nonexistent_guide_test.log");
        let config = LogRotationConfig::default();
        rotate_log_if_needed(path, &config).expect("missing file is not an error");
    }

    #[test]
    fn bearer_token_is_redacted() {
        let content = "Bearer abc123secret token here";
        let redacted = redact_pattern(content, "Bearer ");
        assert!(redacted.contains("Bearer [REDACTED]"));
        assert!(!redacted.contains("abc123secret"));
    }

    #[test]
    fn api_key_is_redacted() {
        let content = "OPENAI_API_KEY=sk-real-key-here";
        let redacted = redact_pattern(content, "OPENAI_API_KEY=");
        assert!(redacted.contains("OPENAI_API_KEY=[REDACTED]"));
        assert!(!redacted.contains("sk-real-key-here"));
    }

    #[test]
    fn non_matching_content_is_unchanged() {
        let content = "normal log entry without secrets";
        let redacted = redact_pattern(content, "Bearer ");
        assert_eq!(redacted, content);
    }

    #[test]
    fn old_rotated_files_are_cleaned_up() {
        let temp = std::env::temp_dir();
        let base = "guide_test_cleanup";
        let path = temp.join(format!("{base}.log"));

        // Create the main log file (large)
        write_test_file(&path, &"x".repeat(200));

        // Create 5 old rotated files
        for i in 0..5u64 {
            let rotated = temp.join(format!("{base}.log.rotated-{i}"));
            write_test_file(&rotated, &format!("old content {i}"));
        }

        let config = LogRotationConfig {
            max_file_bytes: 100,
            max_rotated_files: 2,
        };
        rotate_log_if_needed(&path, &config).expect("rotation succeeds");

        // Count remaining rotated files (should be max_rotated_files + 1 new)
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(&temp) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(&format!("{base}.log.rotated-")) {
                    count += 1;
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let _ = fs::remove_file(&path);
        // max_rotated_files=2 total (the new rotation plus one old)
        assert_eq!(count, 2);
    }

    #[test]
    fn default_config_has_sensible_defaults() {
        let config = LogRotationConfig::default();
        assert_eq!(config.max_file_bytes, 10 * 1024 * 1024);
        assert_eq!(config.max_rotated_files, 5);
    }
}
