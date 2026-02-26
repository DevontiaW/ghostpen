import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";
import Editor from "./components/Editor";
import { highlightEffect } from "./components/Editor";
import IssueSidebar from "./components/IssueSidebar";
import RewritePanel from "./components/RewritePanel";
import type { GrammarIssue, CheckResult } from "./components/Editor";
import type { RewriteResult } from "./components/RewritePanel";
import "./App.css";

interface LlmStatus {
  available: boolean;
  provider: string;
  model: string;
}

function App() {
  const [text, setText] = useState("");
  const [issues, setIssues] = useState<GrammarIssue[]>([]);
  const [stats, setStats] = useState({ word_count: 0, sentence_count: 0, issue_count: 0 });
  const [selectedText, setSelectedText] = useState("");
  const [selectionRange, setSelectionRange] = useState<{ from: number; to: number } | null>(null);
  const [rewriteResult, setRewriteResult] = useState<RewriteResult | null>(null);
  const [rewriteLoading, setRewriteLoading] = useState(false);
  const [llmStatus, setLlmStatus] = useState<LlmStatus>({ available: false, provider: "none", model: "" });
  const [activeMode, setActiveMode] = useState<string | null>(null);
  const editorRef = useRef<ReactCodeMirrorRef | null>(null);

  const scrollToIssue = useCallback((issue: GrammarIssue) => {
    const view = editorRef.current?.view;
    if (!view) return;

    // Scroll the error span into view (centered vertically)
    view.dispatch({
      effects: EditorView.scrollIntoView(
        EditorSelection.range(issue.start, issue.end),
        { y: "center" }
      ),
    });

    // Flash the highlight decoration
    view.dispatch({
      effects: highlightEffect.of({ from: issue.start, to: issue.end }),
    });

    // Clear highlight after 1.5 seconds
    setTimeout(() => {
      // Guard against unmounted editor
      if (editorRef.current?.view) {
        editorRef.current.view.dispatch({
          effects: highlightEffect.of(null),
        });
      }
    }, 1500);
  }, []);

  // Check LLM status on mount + poll every 30s
  useEffect(() => {
    invoke<LlmStatus>("check_llm_status").then(setLlmStatus).catch(console.error);
    const interval = setInterval(() => {
      invoke<LlmStatus>("check_llm_status").then(setLlmStatus).catch(console.error);
    }, 30000);
    return () => clearInterval(interval);
  }, []);

  const handleIssuesFound = useCallback((newIssues: GrammarIssue[], newStats: CheckResult["stats"]) => {
    setIssues(newIssues);
    setStats(newStats);
  }, []);

  const handleSelectionChange = useCallback((selected: string, from: number, to: number) => {
    setSelectedText(selected);
    setSelectionRange(selected ? { from, to } : null);
    if (!selected) setRewriteResult(null);
  }, []);

  const handleRewrite = async (mode: string) => {
    const targetText = selectedText || text;
    if (!targetText.trim()) return;

    setActiveMode(mode);
    setRewriteLoading(true);
    setRewriteResult(null);

    try {
      const result = await invoke<RewriteResult>("rewrite_text", {
        request: { text: targetText, mode },
      });
      setRewriteResult(result);
    } catch (err) {
      setRewriteResult({
        rewritten: "",
        explanation: `Error: ${err}. Make sure Ollama or LM Studio is running locally.`,
      });
    } finally {
      setRewriteLoading(false);
      setActiveMode(null);
    }
  };

  const handleQuickFix = useCallback((pos: number) => {
    const issue = issues.find(i => pos >= i.start && pos <= i.end);
    if (!issue || issue.suggestions.length === 0) return;
    applySuggestion(issue, issue.suggestions[0]);
  }, [issues]);

  const applySuggestion = (issue: GrammarIssue, suggestion: string) => {
    const before = text.substring(0, issue.start);
    const after = text.substring(issue.end);
    const newText = before + suggestion + after;
    setText(newText);
  };

  const applyRewrite = () => {
    if (!rewriteResult?.rewritten) return;
    if (selectionRange) {
      const newText = text.substring(0, selectionRange.from)
        + rewriteResult.rewritten
        + text.substring(selectionRange.to);
      setText(newText);
    } else {
      setText(rewriteResult.rewritten);
    }
    setRewriteResult(null);
    setSelectedText("");
    setSelectionRange(null);
  };

  return (
    <div className="app">
      <div className="header">
        <div className="header-left">
          <div className="logo">Ghost<span>pen</span></div>
          <div className={`status-badge ${llmStatus.available ? "connected" : "disconnected"}`}>
            <div className="status-dot" />
            {llmStatus.available ? `${llmStatus.provider}` : "No LLM"}
          </div>
        </div>
        <div className="stats">
          <div className="stat-item">
            <span className="stat-value">{stats.word_count}</span> words
          </div>
          <div className="stat-item">
            <span className="stat-value">{stats.sentence_count}</span> sentences
          </div>
          <div className="stat-item">
            <span className={`issues-count ${stats.issue_count === 0 ? "clean" : ""}`}>
              {stats.issue_count}
            </span>
            {" "}issues
          </div>
        </div>
      </div>

      <div className="main">
        <div className="editor-panel">
          <div className="editor-toolbar">
            {["clarity", "concise", "formal", "casual"].map((mode) => (
              <button
                key={mode}
                className={`toolbar-btn ${activeMode === mode ? "active" : ""}`}
                onClick={() => handleRewrite(mode)}
                disabled={rewriteLoading || !llmStatus.available}
              >
                {activeMode === mode && <span className="spinner" />}
                {" "}{mode.charAt(0).toUpperCase() + mode.slice(1)}
              </button>
            ))}
            <div className="toolbar-separator" />
            <button
              className={`toolbar-btn ${activeMode === "explain" ? "active" : ""}`}
              onClick={() => handleRewrite("explain")}
              disabled={rewriteLoading || !llmStatus.available}
            >
              {activeMode === "explain" && <span className="spinner" />}
              {" "}Coach Me
            </button>
            {selectedText && (
              <span style={{ fontSize: 12, color: "#7c3aed", marginLeft: 8 }}>
                {selectedText.length} chars selected
              </span>
            )}
          </div>

          <Editor
            value={text}
            onChange={setText}
            onIssuesFound={handleIssuesFound}
            onSelectionChange={handleSelectionChange}
            editorRef={editorRef}
            onQuickFix={handleQuickFix}
          />
        </div>

        <div className="sidebar">
          <IssueSidebar
            issues={issues}
            text={text}
            onApplySuggestion={applySuggestion}
            onScrollToIssue={scrollToIssue}
          />

          <RewritePanel
            rewriteResult={rewriteResult}
            rewriteLoading={rewriteLoading}
            onApply={applyRewrite}
            onDismiss={() => setRewriteResult(null)}
          />
        </div>
      </div>

      <div className="footer">
        <span>Ghostpen v0.2.0 -- Your writing never leaves your machine</span>
        <span>
          {llmStatus.available
            ? `${llmStatus.provider} (${llmStatus.model})`
            : "Install Ollama or LM Studio for AI rewrites"
          }
        </span>
      </div>
    </div>
  );
}

export default App;
