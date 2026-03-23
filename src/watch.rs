use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::{Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::adapter::{all_adapters, Adapter};
use crate::discovery::discover_sessions;
use crate::output::write_events_jsonl;

/// Parse a file from a given byte offset, returning new events and the new EOF position.
/// Returns (events, new_position).
pub fn parse_file_from_offset(
    path: &std::path::Path,
    offset: u64,
    adapter: &dyn Adapter,
    session_id: &str,
    workspace: &str,
) -> Result<(Vec<crate::event::Event>, u64), Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;

    let file_len = file.metadata()?.len();
    if file_len <= offset {
        return Ok((vec![], offset));
    }

    // We need to read new content from offset to end, then parse it.
    // The adapter.parse expects a reader over the full (or partial) content.
    // Since adapters parse line-by-line JSONL, we can feed only the new lines.
    let reader = BufReader::new(&file);
    let mut new_content = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(l) => {
                new_content.push(l);
            }
            Err(_) => break,
        }
    }

    let new_pos = file.stream_position().unwrap_or(file_len);

    if new_content.is_empty() {
        return Ok((vec![], new_pos));
    }

    let joined = new_content.join("\n") + "\n";
    let mut cursor = std::io::Cursor::new(joined.as_bytes().to_vec());
    let events = adapter.parse(&mut cursor, session_id, workspace)?;

    Ok((events, new_pos))
}

/// Update the position map for a file, setting it to current EOF.
pub fn record_file_position(positions: &mut HashMap<PathBuf, u64>, path: &PathBuf) {
    if let Ok(metadata) = std::fs::metadata(path) {
        positions.insert(path.clone(), metadata.len());
    }
}

pub fn run_watch() {
    eprintln!("nose: watching for events... (Ctrl+C to stop)");

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let adapters = all_adapters();
    let mut out = std::io::stdout();

    // Tracks file path -> last read byte offset
    let mut positions: HashMap<PathBuf, u64> = HashMap::new();
    // Tracks file path -> (session_id, workspace, adapter index)
    let mut file_meta: HashMap<PathBuf, (String, String, usize)> = HashMap::new();

    // --- Step 1: Initial parse of all existing content ---
    for (adapter_idx, adapter) in adapters.iter().enumerate() {
        let search_paths = adapter.discovery_paths(&cwd);
        let sessions = discover_sessions(&search_paths, adapter.as_ref());

        for session in sessions {
            let path = &session.path;
            let file = match File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!(
                        "nose: warning: could not open {}: {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
            let mut reader = file;
            match adapter.parse(&mut reader, &session.session_id, &session.workspace) {
                Ok(events) => {
                    if let Err(e) = write_events_jsonl(&events, &mut out) {
                        eprintln!("nose: warning: write error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "nose: warning: failed to parse {}: {}",
                        path.display(),
                        e
                    );
                }
            }

            positions.insert(path.clone(), file_len);
            file_meta.insert(
                path.clone(),
                (
                    session.session_id.clone(),
                    session.workspace.clone(),
                    adapter_idx,
                ),
            );
        }
    }

    let _ = out.flush();

    // --- Step 2: Collect all watch paths ---
    let mut watch_paths: Vec<PathBuf> = Vec::new();
    for adapter in &adapters {
        for p in adapter.discovery_paths(&cwd) {
            if !watch_paths.contains(&p) {
                watch_paths.push(p);
            }
        }
    }

    // --- Step 3: Set up file watcher ---
    let (tx, rx) = mpsc::channel::<notify::Result<NotifyEvent>>();
    let mut watcher: RecommendedWatcher =
        match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("nose: error: could not create watcher: {}", e);
                return;
            }
        };

    for path in &watch_paths {
        if path.exists() {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                eprintln!(
                    "nose: warning: could not watch {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    // --- Step 4: Event loop ---
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                handle_notify_event(
                    event,
                    &adapters,
                    &cwd,
                    &mut positions,
                    &mut file_meta,
                    &mut out,
                );
            }
            Ok(Err(e)) => {
                eprintln!("nose: watcher error: {}", e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No event, keep waiting
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("nose: watcher channel disconnected, exiting");
                break;
            }
        }
    }
}

fn handle_notify_event(
    event: NotifyEvent,
    adapters: &[Box<dyn Adapter>],
    cwd: &std::path::Path,
    positions: &mut HashMap<PathBuf, u64>,
    file_meta: &mut HashMap<PathBuf, (String, String, usize)>,
    out: &mut impl Write,
) {
    let paths = event.paths;

    for path in paths {
        if !path.is_file() {
            continue;
        }

        match event.kind {
            EventKind::Create(_) => {
                // New file: find which adapter handles it
                if let Some((adapter_idx, session_id, workspace)) =
                    detect_adapter_for_file(adapters, cwd, &path)
                {
                    let adapter = &adapters[adapter_idx];
                    let file = match File::open(&path) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("nose: warning: could not open {}: {}", path.display(), e);
                            return;
                        }
                    };
                    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
                    let mut reader = file;
                    match adapter.parse(&mut reader, &session_id, &workspace) {
                        Ok(events) => {
                            if let Err(e) = write_events_jsonl(&events, out) {
                                eprintln!("nose: warning: write error: {}", e);
                            }
                            let _ = out.flush();
                        }
                        Err(e) => {
                            eprintln!(
                                "nose: warning: failed to parse {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                    positions.insert(path.clone(), file_len);
                    file_meta.insert(path, (session_id, workspace, adapter_idx));
                }
            }
            EventKind::Modify(_) => {
                let (session_id, workspace, adapter_idx) =
                    if let Some(meta) = file_meta.get(&path) {
                        meta.clone()
                    } else {
                        // Unknown file - try to detect adapter
                        if let Some((adapter_idx, session_id, workspace)) =
                            detect_adapter_for_file(adapters, cwd, &path)
                        {
                            file_meta.insert(
                                path.clone(),
                                (session_id.clone(), workspace.clone(), adapter_idx),
                            );
                            positions.insert(path.clone(), 0);
                            (session_id, workspace, adapter_idx)
                        } else {
                            return;
                        }
                    };

                let offset = *positions.get(&path).unwrap_or(&0);
                let adapter = &adapters[adapter_idx];

                match parse_file_from_offset(&path, offset, adapter.as_ref(), &session_id, &workspace) {
                    Ok((events, new_pos)) => {
                        if !events.is_empty() {
                            if let Err(e) = write_events_jsonl(&events, out) {
                                eprintln!("nose: warning: write error: {}", e);
                            }
                            let _ = out.flush();
                        }
                        positions.insert(path, new_pos);
                    }
                    Err(e) => {
                        eprintln!("nose: warning: failed to parse {}: {}", path.display(), e);
                    }
                }
            }
            _ => {}
        }
    }
}

fn detect_adapter_for_file(
    adapters: &[Box<dyn Adapter>],
    cwd: &std::path::Path,
    path: &std::path::Path,
) -> Option<(usize, String, String)> {
    for (idx, adapter) in adapters.iter().enumerate() {
        // Check if path is under one of this adapter's discovery paths
        let search_paths = adapter.discovery_paths(cwd);
        let is_in_scope = search_paths
            .iter()
            .any(|sp| path.starts_with(sp));

        if is_in_scope && adapter.detect(path) {
            // Extract session metadata
            let (session_id, workspace) = extract_session_meta(path);
            return Some((idx, session_id, workspace));
        }
    }
    None
}

fn extract_session_meta(path: &std::path::Path) -> (String, String) {
    let fallback_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
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
