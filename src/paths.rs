//! Checks for the file paths the frontend sends to the file commands.
//!
//! These paths come from native open/save dialogs, so the user has already chosen them, but
//! they reach us over IPC and are no more trustworthy than the webview that sent them. Each
//! path is therefore normalised and checked here: absolute, a plain file name, and a parent
//! folder outside the directories that hold the operating system and other programs.

use std::path::{Path, PathBuf};

/// Largest text or audio payload a single command accepts from the webview.
pub const MAX_PAYLOAD_BYTES: usize = 512 * 1024 * 1024;

/// Largest base64 payload, which is what `save_audio` receives.
pub const MAX_PAYLOAD_CHARS: usize = MAX_PAYLOAD_BYTES / 3 * 4;

/// A name made only of letters, digits, `-` and `_`, such as a language or voice id.
pub fn safe_name(value: &str) -> Option<&str> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(value)
}

/// A file the user picked to write to, resolved to its real folder.
///
/// The returned path has no `..` components left, because it is rebuilt from the canonical
/// parent folder and the file name.
pub fn output_file(path: &str) -> Result<PathBuf, String> {
    let requested = absolute(path)?;
    let name = requested
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{path} has no file name"))?;
    if name == "." || name == ".." || is_reserved_device(name) {
        return Err(format!("{path} is not a name a file can have"));
    }
    let parent = requested
        .parent()
        .ok_or_else(|| format!("{path} has no parent folder"))?;
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", parent.display()))?;
    if is_system_directory(&parent) {
        return Err(format!(
            "refusing to write into {}; choose a folder of your own",
            parent.display()
        ));
    }
    let target = parent.join(name);
    if target.is_dir() {
        return Err(format!("{path} is a folder"));
    }
    Ok(target)
}

/// An existing file the user picked to read, resolved to its real path.
pub fn input_file(path: &str) -> Result<PathBuf, String> {
    let requested = absolute(path)?;
    let resolved = requested
        .canonicalize()
        .map_err(|e| format!("cannot open {path}: {e}"))?;
    if !resolved.is_file() {
        return Err(format!("{path} is not a file"));
    }
    Ok(resolved)
}

/// The path as the user would recognise it, without Windows' verbatim prefix.
pub fn display(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

fn absolute(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("the path is empty".into());
    }
    if path.contains('\0') {
        return Err("the path contains a NUL byte".into());
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path", path.display()));
    }
    Ok(path)
}

/// `CON`, `NUL`, `COM1` and friends address devices, not files, on Windows.
fn is_reserved_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

#[cfg(target_os = "windows")]
fn is_system_directory(directory: &Path) -> bool {
    ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"]
        .iter()
        .filter_map(|name| std::env::var_os(name))
        .map(PathBuf::from)
        .map(|root| root.canonicalize().unwrap_or(root))
        .any(|root| directory == root || directory.starts_with(&root))
}

#[cfg(not(target_os = "windows"))]
fn is_system_directory(directory: &Path) -> bool {
    ["/", "/etc", "/usr", "/bin", "/sbin", "/boot", "/dev", "/proc", "/sys"]
        .iter()
        .map(Path::new)
        .any(|root| directory == *root || directory.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_restricted_to_safe_characters() {
        assert_eq!(safe_name("english"), Some("english"));
        assert_eq!(safe_name("green-light_2"), Some("green-light_2"));
        assert_eq!(safe_name(".."), None);
        assert_eq!(safe_name("a/b"), None);
        assert_eq!(safe_name(r"a\b"), None);
        assert_eq!(safe_name(""), None);
        assert_eq!(safe_name(&"a".repeat(65)), None);
    }

    #[test]
    fn relative_and_empty_paths_are_refused() {
        assert!(output_file("").is_err());
        assert!(output_file("relative/output.wav").is_err());
        assert!(input_file("").is_err());
    }

    #[test]
    fn device_names_are_refused() {
        assert!(is_reserved_device("CON"));
        assert!(is_reserved_device("nul.wav"));
        assert!(is_reserved_device("COM3"));
        assert!(!is_reserved_device("console"));
        assert!(!is_reserved_device("comedy"));
    }
}
