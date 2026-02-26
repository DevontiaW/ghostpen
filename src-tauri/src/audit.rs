use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use serde::Serialize;

#[derive(Serialize)]
struct AuditEntry {
    timestamp: String,
    event: String,
    details: serde_json::Value,
}

/// Log an audit event to ~/.ghostpen/logs/audit.jsonl
/// Fire-and-forget: spawns a thread so it never blocks the caller.
pub fn log_event(event: &str, details: serde_json::Value) {
    let event = event.to_string();
    std::thread::spawn(move || {
        let log_dir = dirs::data_local_dir()
            .unwrap_or_default()
            .join("ghostpen")
            .join("logs");
        let _ = create_dir_all(&log_dir);

        let log_file = log_dir.join("audit.jsonl");
        let entry = AuditEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            event,
            details,
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file)
            {
                let _ = writeln!(file, "{}", json);
            }
        }
    });
}
