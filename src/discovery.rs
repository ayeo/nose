use std::io::BufRead;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::adapter::Adapter;

pub struct SessionFile {
    pub path: PathBuf,
    pub session_id: String,
    pub workspace: String,
}

/// Extracts session_id and workspace from the first line of a transcript.
/// Falls back to filename-based session_id and "unknown" workspace if parsing fails.
fn extract_session_metadata(path: &Path) -> (String, String) {
    let fallback_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if let Ok(file) = std::fs::File::open(path) {
        let reader = std::io::BufReader::new(file);
        if let Some(Ok(first_line)) = reader.lines().next() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&first_line) {
                let session_id = v["sessionId"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| fallback_id.clone());
                let workspace = v["cwd"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                return (session_id, workspace);
            }
        }
    }

    (fallback_id, "unknown".to_string())
}

pub fn discover_sessions(search_paths: &[PathBuf], adapter: &dyn Adapter) -> Vec<SessionFile> {
    let mut sessions = Vec::new();

    for base_path in search_paths {
        if !base_path.exists() {
            continue;
        }

        for entry in WalkDir::new(base_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && adapter.detect(path) {
                let (session_id, workspace) = extract_session_metadata(path);

                sessions.push(SessionFile {
                    path: path.to_path_buf(),
                    session_id,
                    workspace,
                });
            }
        }
    }

    sessions
}
