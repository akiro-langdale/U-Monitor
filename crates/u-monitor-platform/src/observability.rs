//! Tracing helpers shared by platform backends. Subscriber init lives in the UI crate.

use std::path::Path;

const SENSITIVE_MARKERS: &[&str] = &[
    ".env",
    "secret",
    "credential",
    "password",
    "passwd",
    "token",
    "apikey",
    "api_key",
    "private_key",
];

/// Replaces path segments that look like secrets before they are logged.
pub fn scrub_path_for_log(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if SENSITIVE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return "[redacted-path]".into();
    }
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("…/{name}"))
        .unwrap_or_else(|| "[path]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_paths_are_redacted() {
        assert_eq!(
            scrub_path_for_log(r"C:\secrets\api_key.txt"),
            "[redacted-path]"
        );
    }

    #[test]
    fn ordinary_paths_keep_basename_only() {
        assert_eq!(
            scrub_path_for_log(r"/var/log/u-monitor/app.log"),
            "…/app.log"
        );
    }
}
