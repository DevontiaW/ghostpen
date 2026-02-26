# Why Ghostpen Exists

In 2024, Grammarly launched a feature called Authorship that tracks every keystroke, paste, and deletion you make in a document. It captures your deleted thoughts, your AI prompts, and packages them into a shareable report. Institutions can deploy it campus-wide. Students cannot refuse it without refusing to have their work graded.

The company that spent a decade helping writers find their voice is now selling the record of how they found it.

Ghostpen is the response.

## What We Believe

**Privacy is architectural, not policy.** Ghostpen runs entirely on your machine. There is no server to send data to. There is no telemetry. There is no "opt-in" surveillance dressed as empowerment. Your writing never leaves your device because there is nowhere for it to go. This is not a promise. It is a design constraint enforced by the code, which you can read.

**Teaching beats policing.** The best version of Grammarly wasn't the one that flagged your mistakes. It was the one that taught you why they were mistakes. Ghostpen explains every suggestion — not just what to change, but why it matters. The goal is to make you a better writer, not a more surveilled one.

**Open source means it can't be weaponized.** If someone forks Ghostpen and adds surveillance, the community sees it. The transparent codebase means no one can hide what they changed. You own this tool the way you own a pen.

## The Problem This Solves

Read the full argument: ["The Integrity Theater"](https://notesofanomad.substack.com/) on Substack.

The short version: AI detection tools have false-positive rates between 5-20%. They disproportionately flag non-native English speakers. They cannot distinguish between cheating and disability accommodation. They are deployed by institutions that simultaneously encourage AI use. And the company selling the most sophisticated surveillance tool is the same company whose core product is AI-assisted writing.

Students, professionals, and anyone who writes deserves a tool that is unambiguously on their side. Ghostpen is that tool.

## How It Works

- **Harper** (Rust) handles grammar, spelling, and punctuation checks instantly, on-device
- **Your local LLM** (Ollama or LM Studio) handles AI rewrites, tone adjustment, and writing coaching
- **Tauri** provides a lightweight desktop app (~10MB, not Electron's 100MB+)
- **Zero network calls.** Verify this yourself — the source is right here

## License

MIT. Use it however you want. Build on it. Make it better.

---

*Built by [Textstone Labs](https://textstonelabs.com). Fueled by justified rage.*
