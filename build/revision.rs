use std::{env, path::Path, process::Command};

pub fn resolve(root: &Path) -> Result<String, String> {
    let git = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())
        .and_then(|output| {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
            }
        });
    select(
        git,
        [
            env::var("ENLUMI_BUILD_COMMIT").ok(),
            env::var("GITHUB_SHA").ok(),
        ],
    )
}

fn select(git: Result<String, String>, fallback: [Option<String>; 2]) -> Result<String, String> {
    git.as_ref()
        .ok()
        .into_iter()
        .chain(fallback.iter().flatten())
        .map(|sha| sha.trim())
        .find(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "Portable updater needs Git revision metadata: Git: {}; ENLUMI_BUILD_COMMIT and GITHUB_SHA missing or invalid (expected 40 hex digits)",
                git.as_ref().err().map_or("invalid HEAD hash", String::as_str)
            )
        })
}

#[cfg(test)]
mod tests {
    use super::select;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn container_without_git_metadata_uses_github_sha() {
        assert_eq!(
            select(Err("dubious ownership".into()), [None, Some(SHA.into())]),
            Ok(SHA.into())
        );
    }

    #[test]
    fn explicit_revision_supports_source_archives_and_checkout_wins_when_available() {
        assert_eq!(
            select(Err("not a repository".into()), [Some(SHA.into()), None]),
            Ok(SHA.into())
        );
        assert_eq!(
            select(Ok(SHA.into()), [Some("f".repeat(40)), None]),
            Ok(SHA.into())
        );
    }

    #[test]
    fn invalid_revision_fails_with_original_git_error() {
        let error =
            select(Err("dubious ownership".into()), [Some("abc".into()), None]).unwrap_err();
        assert!(error.contains("dubious ownership"));
        assert!(error.contains("GITHUB_SHA missing or invalid"));
    }
}
