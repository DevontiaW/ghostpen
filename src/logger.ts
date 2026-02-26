// Simple frontend event logger -- writes to console + localStorage ring buffer
const MAX_LOG_ENTRIES = 500;
const LOG_KEY = "ghostpen_log";

interface LogEntry {
  timestamp: string;
  event: string;
  details: Record<string, unknown>;
}

export function logEvent(event: string, details: Record<string, unknown> = {}) {
  const entry: LogEntry = {
    timestamp: new Date().toISOString(),
    event,
    details,
  };

  console.log(`[Ghostpen] ${event}`, details);

  // Ring buffer in localStorage
  try {
    const existing = JSON.parse(localStorage.getItem(LOG_KEY) || "[]") as LogEntry[];
    existing.push(entry);
    if (existing.length > MAX_LOG_ENTRIES) {
      existing.splice(0, existing.length - MAX_LOG_ENTRIES);
    }
    localStorage.setItem(LOG_KEY, JSON.stringify(existing));
  } catch { /* localStorage full or unavailable */ }
}
