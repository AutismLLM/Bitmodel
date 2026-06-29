//! File-extension allowlist — an anti-abuse/format guard, NOT a license check.
//! BitModel mirrors public model files; the only content filter is the extension.

/// Extensions BitModel will mirror (lower-cased, without the leading dot).
pub const ALLOWED_EXTENSIONS: &[&str] = &[
    "safetensors", "gguf", "bin", "pt", "pth", "onnx", "ggml", "q4_0", "q8_0",
];

/// True if `path`'s final extension is on the allowlist.
pub fn is_allowed(path: &str) -> bool {
    match extension_of(path) {
        Some(ext) => ALLOWED_EXTENSIONS.contains(&ext.as_str()),
        None => false,
    }
}

/// Lower-cased extension after the last `.` in the final path component.
fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let (_, ext) = name.rsplit_once('.')?;
    if ext.is_empty() {
        None
    } else {
        Some(ext.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_model_extensions() {
        assert!(is_allowed("model.safetensors"));
        assert!(is_allowed("a/b/c/model-00001-of-00002.safetensors"));
        assert!(is_allowed("Qwen2.5-1.5B.GGUF")); // case-insensitive
        assert!(is_allowed("weights.bin"));
    }

    #[test]
    fn rejects_everything_else() {
        assert!(!is_allowed("README.md"));
        assert!(!is_allowed("malware.sh"));
        assert!(!is_allowed("config.json"));
        assert!(!is_allowed("noextension"));
        assert!(!is_allowed("trailingdot."));
    }
}
