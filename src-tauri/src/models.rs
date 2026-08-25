use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    /// Absolute path to the .gguf file
    pub path: String,
    /// Path relative to models_dir (unique key for UI)
    pub relative_path: String,
    /// File stem used as display / alias base
    pub name: String,
    /// First path segment under models_dir (for submenu grouping)
    pub group: String,
}

pub fn scan_models(models_dir: &str) -> Result<Vec<ModelEntry>, String> {
    let root = PathBuf::from(models_dir.trim());
    if models_dir.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !root.is_dir() {
        return Err(format!("Models directory not found: {}", root.display()));
    }

    let mut entries = Vec::new();
    for entry in WalkDir::new(&root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if ext.as_deref() != Some("gguf") {
            continue;
        }

        let relative = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model")
            .to_string();

        let group = relative
            .split('/')
            .next()
            .filter(|s| !s.is_empty() && relative.contains('/'))
            .unwrap_or("(root)")
            .to_string();

        entries.push(ModelEntry {
            path: path.to_string_lossy().to_string(),
            relative_path: relative,
            name,
            group,
        });
    }

    entries.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    Ok(entries)
}
