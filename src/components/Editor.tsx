import { useCallback, useEffect } from "react";
import CodeMirror, { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView, Decoration, type DecorationSet } from "@codemirror/view";
import { StateEffect, StateField } from "@codemirror/state";
import { linter, type Diagnostic, lintGutter } from "@codemirror/lint";
import { invoke } from "@tauri-apps/api/core";

interface GrammarIssue {
  start: number;
  end: number;
  message: string;
  suggestions: string[];
  severity: string;
}

interface CheckResult {
  issues: GrammarIssue[];
  stats: {
    word_count: number;
    sentence_count: number;
    issue_count: number;
  };
}

interface EditorProps {
  value: string;
  onChange: (value: string) => void;
  onIssuesFound: (issues: GrammarIssue[], stats: CheckResult["stats"]) => void;
  onSelectionChange: (text: string) => void;
  editorRef: React.RefObject<ReactCodeMirrorRef | null>;
}

// --- Highlight effect system for sidebar-to-editor sync ---
const highlightEffect = StateEffect.define<{ from: number; to: number } | null>();

const highlightMark = Decoration.mark({ class: "cm-issue-highlight" });

const highlightField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, tr) {
    for (const e of tr.effects) {
      if (e.is(highlightEffect)) {
        if (e.value === null) return Decoration.none;
        return Decoration.set([
          highlightMark.range(e.value.from, e.value.to),
        ]);
      }
    }
    return decorations;
  },
  provide: (f) => EditorView.decorations.from(f),
});

function mapSeverity(severity: string): "error" | "warning" | "info" {
  const s = severity.toLowerCase();
  if (s.includes("error") || s.includes("spell")) return "error";
  if (s.includes("style") || s.includes("readability")) return "info";
  return "warning";
}

// Store latest issues callback in a ref so the linter closure can access it
let issuesCallback: ((issues: GrammarIssue[], stats: CheckResult["stats"]) => void) | null = null;

const grammarLinter = linter(
  async (view) => {
    const text = view.state.doc.toString();
    if (!text.trim()) {
      if (issuesCallback) {
        issuesCallback([], { word_count: 0, sentence_count: 0, issue_count: 0 });
      }
      return [];
    }

    try {
      const result = await invoke<CheckResult>("check_grammar", { text });

      if (issuesCallback) {
        issuesCallback(result.issues, result.stats);
      }

      return result.issues.map((issue): Diagnostic => ({
        from: issue.start,
        to: issue.end,
        severity: mapSeverity(issue.severity),
        message: issue.message,
        actions: issue.suggestions.map((s) => ({
          name: s,
          apply: (view: EditorView) => {
            view.dispatch({
              changes: { from: issue.start, to: issue.end, insert: s },
            });
          },
        })),
      }));
    } catch (err) {
      console.error("Grammar check failed:", err);
      return [];
    }
  },
  { delay: 300 }
);

const ghostpenTheme = EditorView.theme({
  "&": {
    fontSize: "16px",
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    height: "100%",
  },
  ".cm-content": {
    lineHeight: "1.8",
    padding: "24px",
    caretColor: "#7c3aed",
  },
  ".cm-focused": {
    outline: "none",
  },
  ".cm-scroller": {
    overflow: "auto",
  },
  ".cm-gutters": {
    display: "none",
  },
  // Squiggly underlines
  ".cm-lintRange-error": {
    backgroundImage: "none",
    textDecoration: "underline wavy #ef4444",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  ".cm-lintRange-warning": {
    backgroundImage: "none",
    textDecoration: "underline wavy #f59e0b",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  ".cm-lintRange-info": {
    backgroundImage: "none",
    textDecoration: "underline wavy #7c3aed",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  // Lint tooltip styling
  ".cm-tooltip-lint": {
    background: "#1a1a24",
    border: "1px solid #2d2d3a",
    borderRadius: "8px",
    padding: "8px 12px",
    color: "#e5e7eb",
    fontSize: "13px",
    maxWidth: "400px",
  },
  ".cm-lint-marker-error": {
    content: "none",
  },
  ".cm-diagnostic-error": {
    borderLeft: "3px solid #ef4444",
    padding: "4px 8px",
  },
  ".cm-diagnostic-warning": {
    borderLeft: "3px solid #f59e0b",
    padding: "4px 8px",
  },
  ".cm-diagnostic-info": {
    borderLeft: "3px solid #7c3aed",
    padding: "4px 8px",
  },
  ".cm-lintPoint::after": {
    display: "none",
  },
  // Action buttons in lint tooltip
  ".cm-diagnostic .cm-diagnosticAction": {
    background: "#7c3aed",
    color: "#fff",
    border: "none",
    borderRadius: "4px",
    padding: "2px 8px",
    fontSize: "12px",
    fontWeight: "500",
    cursor: "pointer",
    marginLeft: "4px",
  },
}, { dark: false });

const ghostpenDarkTheme = EditorView.theme({
  "&": {
    fontSize: "16px",
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    height: "100%",
    backgroundColor: "transparent",
  },
  ".cm-content": {
    lineHeight: "1.8",
    padding: "24px",
    caretColor: "#7c3aed",
    color: "#e5e7eb",
  },
  ".cm-focused": {
    outline: "none",
  },
  ".cm-scroller": {
    overflow: "auto",
  },
  ".cm-gutters": {
    display: "none",
  },
  ".cm-cursor": {
    borderLeftColor: "#7c3aed",
  },
  ".cm-selectionBackground": {
    background: "#7c3aed33 !important",
  },
  // Squiggly underlines
  ".cm-lintRange-error": {
    backgroundImage: "none",
    textDecoration: "underline wavy #ef4444",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  ".cm-lintRange-warning": {
    backgroundImage: "none",
    textDecoration: "underline wavy #f59e0b",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  ".cm-lintRange-info": {
    backgroundImage: "none",
    textDecoration: "underline wavy #7c3aed",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  // Lint tooltip styling
  ".cm-tooltip-lint": {
    background: "#1a1a24",
    border: "1px solid #2d2d3a",
    borderRadius: "8px",
    padding: "8px 12px",
    color: "#e5e7eb",
    fontSize: "13px",
    maxWidth: "400px",
  },
  ".cm-diagnostic-error": {
    borderLeft: "3px solid #ef4444",
    padding: "4px 8px",
  },
  ".cm-diagnostic-warning": {
    borderLeft: "3px solid #f59e0b",
    padding: "4px 8px",
  },
  ".cm-diagnostic-info": {
    borderLeft: "3px solid #7c3aed",
    padding: "4px 8px",
  },
  ".cm-lintPoint::after": {
    display: "none",
  },
  ".cm-diagnostic .cm-diagnosticAction": {
    background: "#7c3aed",
    color: "#fff",
    border: "none",
    borderRadius: "4px",
    padding: "2px 8px",
    fontSize: "12px",
    fontWeight: "500",
    cursor: "pointer",
    marginLeft: "4px",
  },
}, { dark: true });

export default function Editor({ value, onChange, onIssuesFound, onSelectionChange, editorRef }: EditorProps) {
  const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

  // Keep the issues callback in sync
  useEffect(() => {
    issuesCallback = onIssuesFound;
    return () => { issuesCallback = null; };
  }, [onIssuesFound]);

  const handleChange = useCallback((val: string) => {
    onChange(val);
  }, [onChange]);

  // Track selection changes via update listener
  const selectionListener = EditorView.updateListener.of((update) => {
    if (update.selectionSet) {
      const { from, to } = update.state.selection.main;
      if (from !== to) {
        const selected = update.state.sliceDoc(from, to);
        onSelectionChange(selected);
      } else {
        onSelectionChange("");
      }
    }
  });

  const extensions = [
    grammarLinter,
    lintGutter(),
    selectionListener,
    highlightField,
    isDark ? ghostpenDarkTheme : ghostpenTheme,
    EditorView.lineWrapping,
  ];

  return (
    <div className="editor-container">
      <CodeMirror
        ref={editorRef}
        value={value}
        onChange={handleChange}
        extensions={extensions}
        placeholder="Start writing, or paste your text here.

Ghostpen checks grammar instantly using Harper — everything stays on your machine.

Select text and click a rewrite button to get AI-powered suggestions from your local LLM (Ollama or LM Studio).

No cloud. No surveillance. No keystrokes logged. Just better writing."
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          highlightActiveLine: false,
          highlightSelectionMatches: true,
          bracketMatching: false,
          autocompletion: false,
          indentOnInput: false,
        }}
        theme={isDark ? "dark" : "light"}
      />
    </div>
  );
}

export { highlightEffect };
export type { GrammarIssue, CheckResult };
