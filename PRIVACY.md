# Privacy Architecture

Ghostpen is designed so that your writing **cannot** leave your machine. This is not a policy. It is an architectural constraint.

## What Ghostpen Does NOT Do

- Does not connect to any external server
- Does not send telemetry, analytics, or crash reports
- Does not track keystrokes, clipboard activity, or typing patterns
- Does not store writing history or replay data
- Does not generate "authorship reports" or any shareable surveillance artifact
- Does not phone home for license checks, updates, or feature flags
- Does not have an admin dashboard, institutional deployment mode, or any mechanism for a third party to monitor your writing

## What Ghostpen DOES Do

- Runs Harper (grammar checker) entirely in-process — no network required
- Connects to YOUR local LLM server (Ollama or LM Studio) on localhost only
- Stores writing statistics (word count, issue patterns) in a local SQLite database on your machine
- All data stays in your user directory. No cloud sync. No shared storage.

## How to Verify

The entire codebase is open source. To verify there are no network calls:

1. Read `src-tauri/src/llm.rs` — the only HTTP calls go to `localhost:11434` (Ollama) or `localhost:1234` (LM Studio)
2. Read `src-tauri/src/lib.rs` — Harper runs as a Rust library, no network involvement
3. Run the app with a network monitor (Wireshark, Little Snitch, etc.) — you will see zero external connections
4. Build from source yourself: `npm run tauri build`

## Comparison with Grammarly Authorship

| Feature | Grammarly Authorship | Ghostpen |
|---------|---------------------|----------|
| Keystroke logging | Yes | No |
| Clipboard monitoring | Yes | No |
| Deleted text capture | Yes | No |
| AI prompt capture | Yes | No |
| Shareable reports | Yes | No |
| Admin dashboards | Yes | No |
| Server-side storage | 12 months | None |
| Network connections | Required | None (except local LLM) |
| Source code | Closed | Open (MIT) |

## The Principle

If a writing tool requires you to trust a company's privacy policy, you're already in the wrong architecture. Ghostpen doesn't ask for your trust. It asks you to read the code.
