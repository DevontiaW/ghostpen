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
- **Punctuation rules** — Catches double spaces, repeated punctuation (!! ??), and sentences missing ending punctuation. Shown as purple style underlines

### AI Rewrites (Local LLM)
- **5 rewrite modes** — Clarity, Concise, Formal, Casual, and Coach Me (explains WHY changes improve your writing)
- **Streaming responses** — See tokens as they arrive with live markdown rendering. Cancel anytime
- **Selection-aware** — Select specific text to rewrite just that portion, or rewrite the whole document
- **Works with Ollama or LM Studio** — Bring your own model. Auto-detects which is running and shows the loaded model name
- **Smart model detection** — Automatically picks a chat model (skips embedding models) from LM Studio
- **LM Studio auto-launch** — One-click button to start LM Studio if it's installed but not running

### File Management
- **Open files** — Ctrl+O opens `.txt`, `.md`, and other text files. Filename shown in header
- **Save / Save As** — Ctrl+S saves to the current file (or prompts Save As for new documents). Ctrl+Shift+S always prompts a new location
- **Dirty indicator** — Yellow dot next to the filename when you have unsaved changes
- **Auto-save draft** — Text is automatically saved to localStorage every second. Close the app accidentally? Your text is restored on next launch
- **Copy all** — One-click button copies your entire document to the clipboard
- **Export** — Save your work as `.txt` or `.md` via the Export button in the toolbar

### First-Launch Onboarding
- **3-step wizard** — Appears on first launch to welcome you, detect your LLM setup, and show keyboard shortcuts
- **LLM auto-detection** — Checks for Ollama (port 11434) and LM Studio (port 1234). Shows status and download links if not found
- **Non-blocking** — Grammar checking works without an LLM. The wizard lets you skip AI setup and come back later

### Quality Infrastructure
- **Feedback loop** — Rate rewrites (Good/Bad) to build a local dataset for tracking quality
- **Audit logging** — Every grammar check, rewrite, and action logged locally for debugging and accuracy measurement
- **Grammar accuracy baseline** — Test corpus with 20 sentences, 41 known issues. Harper baseline: 70.7% recall, 97.1% precision
- **Frontend event log** — Ring buffer in localStorage (500 entries) for debugging user-facing issues

### Privacy & Performance
- **Zero network calls** — Everything runs on localhost. Verify it yourself
- **Content Security Policy** — CSP locked down to `self` + localhost LLM ports. No inline scripts
- **Dark mode** — Respects your system theme preference
- **Tiny footprint** — ~25MB binary, ~50MB RAM idle (not Electron's 300MB+)
- **Cross-platform** — Windows, macOS, Linux via Tauri

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (for building)
- [Node.js](https://nodejs.org/) 18+ (for frontend)
- [Ollama](https://ollama.ai/) or [LM Studio](https://lmstudio.ai/) (for AI rewrites — optional, grammar works without it)

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

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| **Ctrl+O** | Open a text file |
| **Ctrl+S** | Save (or Save As if no file open) |
| **Ctrl+Shift+S** | Save As (always prompts for location) |
| **Ctrl+.** | Quick-fix the issue at cursor position |

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
|  |  <10ms      |    |  SSE streaming |   |
|  +-------------+    +----------------+   |
|                                          |
|  +-------------+    +----------------+   |
|  | Punctuation |    |  File I/O      |   |
|  | (Regex)     |    |  (Dialog+FS)   |   |
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
| Frontend | React 19 + TypeScript |
| Code editor | [CodeMirror 6](https://codemirror.net/) via @uiw/react-codemirror |
| Grammar engine | [Harper](https://github.com/Automattic/harper) (Rust, in-process) |
| Punctuation checks | Regex-based rules (Rust, in-process) |
| LLM inference | [Ollama](https://ollama.ai/) or [LM Studio](https://lmstudio.ai/) |
| File operations | Tauri plugins (dialog, fs, clipboard-manager) |
| Audit/feedback | Local JSONL files (never leaves your machine) |

## Data Storage

All data stays on your machine:

| Data | Location | Purpose |
|------|----------|---------|
| Audit logs | `%LOCALAPPDATA%/ghostpen/logs/audit.jsonl` | Debugging, accuracy tracking |
| Feedback | `~/.ghostpen/feedback.jsonl` | Rewrite quality ratings |
| Draft auto-save | Browser localStorage | Recovers text if app closes unexpectedly |
| Frontend logs | Browser localStorage | UI event debugging |
| Onboarding state | Browser localStorage | Tracks whether wizard has been shown |

No telemetry. No analytics. No phone-home. Ever.

## Contributing

PRs welcome. The codebase is intentionally simple — if you can read React and basic Rust, you can contribute.

Priority areas:
- Model validation (detect broken/garbage output early)
- Writing pattern tracking over time
- Custom style rules
- Multi-language support
- Browser extension (privacy-first, no surveillance capabilities)
- Undo history for applied rewrites

## Known Limitations

- **LLM quality varies** — Small local models (3B) can produce inconsistent output. 8B+ models recommended. CPU inference on 13B+ will be slow
- **No undo for applied rewrites** — Ctrl+Z works for grammar fixes but applying a full-document rewrite replaces the entire document
- **Punctuation rules are basic** — Double spaces, repeated punctuation, and missing periods are caught. Comma splices and complex punctuation require the Coach Me mode

## Changelog

### v0.3.0 (2026-03-01)
- File Open (Ctrl+O), Save (Ctrl+S), Save As (Ctrl+Shift+S)
- Auto-save drafts to localStorage (text persists across sessions)
- Copy and Export buttons in toolbar
- Dirty indicator (yellow dot) for unsaved changes
- First-launch onboarding wizard with LLM auto-detection
- Basic punctuation rules (double spaces, repeated punctuation, missing periods)
- Filename display in header

### v0.2.0
- All 6 critical bug fixes (UTF-8 offsets, CM6 dispatch, LLM parse, CSP, streaming+cancel, model detection)
- Markdown rendering in rewrite panel
- Scrollable rewrite panel
- Section labels for rewrite suggestions
- Feedback system (Good/Bad ratings)
- Coach Me mode

### v0.1.0
- Initial release: Harper grammar checking, CodeMirror editor, Ollama/LM Studio integration

## License

MIT. See [LICENSE](LICENSE).

---

Built by [Textstone Labs](https://textstonelabs.com).
