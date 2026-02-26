# Ghostpen

**Open-source, local-first writing assistant. Your writing never leaves your machine.**

Ghostpen is a desktop writing tool that checks grammar instantly and offers AI-powered rewrites — all running locally on your computer. No cloud. No surveillance. No keystrokes logged.

## Features

- **Instant grammar checking** — Powered by [Harper](https://writewithharper.com/) (Rust), checks happen in <10ms
- **AI rewrites** — Clarity, conciseness, tone adjustment via your local LLM
- **Writing coach** — Explains WHY changes improve your writing, not just what to change
- **Works with Ollama or LM Studio** — Bring your own model (recommended: Qwen 2.5 3B)
- **Zero network calls** — Everything runs on localhost. Verify it yourself
- **Tiny footprint** — ~10MB install, ~50MB RAM idle (not Electron's 100MB+)
- **Cross-platform** — Windows, macOS, Linux

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (for building)
- [Node.js](https://nodejs.org/) 18+ (for frontend)
- [Ollama](https://ollama.ai/) or [LM Studio](https://lmstudio.ai/) (for AI rewrites — optional)

### Install & Run

```bash
git clone https://github.com/textstonelabs/ghostpen.git
cd ghostpen
npm install
npm run tauri dev
```

### Set Up Local LLM (Optional)

For AI-powered rewrites, install Ollama and pull a model:

```bash
# Install Ollama from https://ollama.ai
ollama pull qwen2.5:3b
```

Or use LM Studio — download any model and start the local server.

Ghostpen auto-detects which is running.

## Why This Exists

Read [The Integrity Theater](https://notesofanomad.substack.com/) — the article that started this project.

Short version: Grammarly's Authorship feature logs every keystroke, captures deleted thoughts, and packages your writing process into surveillance reports for institutions. Students can't opt out. The tool can't distinguish cheating from disability accommodation. And the company selling it is the same one whose core product is AI-assisted writing.

Ghostpen is the tool that should have existed instead.

## Architecture

```
+------------------+     +------------------+
|   React Frontend |     |   Issue Sidebar  |
|   (Text Editor)  |     |   (Suggestions)  |
+--------+---------+     +--------+---------+
         |                         |
         v                         v
+------------------------------------------+
|          Tauri (Rust Backend)            |
|                                          |
|  +-------------+    +----------------+   |
|  |   Harper    |    |  LLM Client    |   |
|  |  (Grammar)  |    | (Ollama/LMS)   |   |
|  |  <10ms      |    |  localhost      |   |
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
| Grammar engine | [Harper](https://github.com/Automattic/harper) (Rust) |
| LLM inference | [Ollama](https://ollama.ai/) or [LM Studio](https://lmstudio.ai/) |
| Editor | Native textarea (TipTap planned for v0.2) |

## Contributing

PRs welcome. The codebase is intentionally simple — if you can read React and basic Rust, you can contribute.

Priority areas:
- Rich text editor with inline annotations (TipTap/CodeMirror)
- Writing pattern tracking over time
- Custom style rules
- Multi-language support
- Browser extension (privacy-first, no surveillance capabilities)

## License

MIT. See [LICENSE](LICENSE).

---

Built by [Textstone Labs](https://textstonelabs.com).
