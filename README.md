# Ghostpen

**Open-source, local-first writing assistant. Your writing never leaves your machine.**

Ghostpen is a desktop writing tool that checks grammar instantly and offers AI-powered rewrites — all running locally on your computer. No cloud. No surveillance. No keystrokes logged.

![Ghostpen Editor](screenshots/editor.png)
*Screenshot coming soon*

## Features

### Grammar & Style (Instant, No Network)
- **Squiggly underlines** — Red (errors/spelling), yellow (warnings), purple (style/readability) highlight issues inline as you type
- **Sidebar issue cards** — Click any issue to scroll the editor to the error with a purple highlight flash
- **One-click fixes** — Apply suggestions from hover tooltips or sidebar chips
- **Ctrl+. quick-fix** — Keyboard shortcut applies the first suggestion at your cursor position
- **Instant checking** — Powered by [Harper](https://writewithharper.com/) (Rust), checks happen in under 10ms

### AI Rewrites (Local LLM)
- **5 rewrite modes** — Clarity, Concise, Formal, Casual, and Coach Me (explains WHY changes improve your writing)
- **Selection-aware** — Select specific text to rewrite just that portion, or rewrite the whole document
- **Works with Ollama or LM Studio** — Bring your own model. Auto-detects which is running
- **LM Studio auto-launch** — One-click button to start LM Studio if it's installed but not running

### Quality Infrastructure
- **Feedback loop** — Rate rewrites (Good/Bad) to build a local dataset for tracking quality
- **Audit logging** — Every grammar check, rewrite, and action logged locally for debugging and accuracy measurement
- **Grammar accuracy baseline** — Test corpus with 20 sentences, 41 known issues. Harper baseline: 70.7% recall, 97.1% precision
- **Frontend event log** — Ring buffer in localStorage (500 entries) for debugging user-facing issues

### Privacy & Performance
- **Zero network calls** — Everything runs on localhost. Verify it yourself
- **Dark mode** — Respects your system theme preference
- **Tiny footprint** — ~25MB binary, ~50MB RAM idle (not Electron's 300MB+)
- **Cross-platform** — Windows, macOS, Linux via Tauri

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (for building)
- [Node.js](https://nodejs.org/) 18+ (for frontend)
- [Ollama](https://ollama.ai/) or [LM Studio](https://lmstudio.ai/) (for AI rewrites — optional)

### Install & Run

```bash
git clone https://github.com/DevontiaW/ghostpen.git
cd ghostpen
npm install
npm run tauri dev
```

### Build for Distribution

```bash
npm run tauri build
# Produces: .exe, .msi installer, and NSIS setup
```

### Set Up Local LLM (Optional)

For AI-powered rewrites, install Ollama and pull a model:

```bash
# Install Ollama from https://ollama.ai
ollama pull qwen2.5:3b
```

Or use LM Studio — download any model and start the local server. Ghostpen auto-detects which is running.

**Note:** Model quality matters. Small models (3B-8B) work but may produce inconsistent output. We're actively testing which models give the best writing assistance results.

## Why This Exists

Read [The Integrity Theater](https://notesofanomad.substack.com/) — the article that started this project.

Short version: Grammarly's Authorship feature logs every keystroke, captures deleted thoughts, and packages your writing process into surveillance reports for institutions. Students can't opt out. The tool can't distinguish cheating from disability accommodation. And the company selling it is the same one whose core product is AI-assisted writing.

Ghostpen is the tool that should have existed instead.

See also: [MANIFESTO.md](MANIFESTO.md) and [PRIVACY.md](PRIVACY.md)

## Architecture

```
+------------------+     +------------------+
|   React Frontend |     |   Issue Sidebar  |
|  (CodeMirror 6)  |     |   + Rewrite Panel|
+--------+---------+     +--------+---------+
         |                         |
         v                         v
+------------------------------------------+
|         Tauri 2.0 (Rust Backend)         |
|                                          |
|  +-------------+    +----------------+   |
|  |   Harper    |    |  LLM Client    |   |
|  |  (Grammar)  |    | (Ollama/LMS)   |   |
|  |  <10ms      |    |  127.0.0.1     |   |
|  +-------------+    +----------------+   |
|                                          |
|  +-------------+    +----------------+   |
|  |   Audit     |    |  Feedback      |   |
|  |  (JSONL)    |    |  (JSONL)       |   |
|  +-------------+    +----------------+   |
+------------------------------------------+
         |                    |
    [In-process]      [localhost only]
    No network         No external calls
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Desktop shell | [Tauri 2.0](https://tauri.app/) |
| Frontend | React + TypeScript |
| Code editor | [CodeMirror 6](https://codemirror.net/) via @uiw/react-codemirror |
| Grammar engine | [Harper](https://github.com/Automattic/harper) (Rust, in-process) |
| LLM inference | [Ollama](https://ollama.ai/) or [LM Studio](https://lmstudio.ai/) |
| Audit/feedback | Local JSONL files (never leaves your machine) |

## Data Storage

All data stays on your machine:

| Data | Location | Purpose |
|------|----------|---------|
| Audit logs | `%LOCALAPPDATA%/ghostpen/logs/audit.jsonl` | Debugging, accuracy tracking |
| Feedback | `~/.ghostpen/feedback.jsonl` | Rewrite quality ratings |
| Frontend logs | Browser localStorage | UI event debugging |

No telemetry. No analytics. No phone-home. Ever.

## Contributing

PRs welcome. The codebase is intentionally simple — if you can read React and basic Rust, you can contribute.

Priority areas:
- Streaming LLM responses (show tokens as they arrive)
- Model validation (detect broken/garbage output early)
- Writing pattern tracking over time
- Custom style rules
- Multi-language support
- Browser extension (privacy-first, no surveillance capabilities)

## Known Limitations

- **UTF-8 vs UTF-16 offsets** — Harper uses byte offsets, JavaScript uses code units. Emoji and CJK characters may cause misaligned underlines
- **LLM quality varies** — Small local models can produce inconsistent output. Model selection and validation is an active area of development
- **No streaming yet** — Rewrite requests block until complete (can take 1-3 minutes on CPU with larger models)

## License

MIT. See [LICENSE](LICENSE).

---

Built by [Textstone Labs](https://textstonelabs.com).
