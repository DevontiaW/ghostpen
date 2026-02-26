import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

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

interface RewriteResult {
  rewritten: string;
  explanation: string;
}

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
  const [rewriteResult, setRewriteResult] = useState<RewriteResult | null>(null);
  const [rewriteLoading, setRewriteLoading] = useState(false);
  const [llmStatus, setLlmStatus] = useState<LlmStatus>({ available: false, provider: "none", model: "" });
  const [activeMode, setActiveMode] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Check LLM status on mount
  useEffect(() => {
    invoke<LlmStatus>("check_llm_status").then(setLlmStatus).catch(console.error);
    const interval = setInterval(() => {
      invoke<LlmStatus>("check_llm_status").then(setLlmStatus).catch(console.error);
    }, 30000);
    return () => clearInterval(interval);
  }, []);

  // Debounced grammar check
  const checkGrammar = useCallback(async (value: string) => {
    if (!value.trim()) {
      setIssues([]);
      setStats({ word_count: 0, sentence_count: 0, issue_count: 0 });
      return;
    }
    try {
      const result = await invoke<CheckResult>("check_grammar", { text: value });
      setIssues(result.issues);
      setStats(result.stats);
    } catch (err) {
      console.error("Grammar check failed:", err);
    }
  }, []);

  const handleTextChange = (value: string) => {
    setText(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => checkGrammar(value), 300);
  };

  const handleSelect = () => {
    const el = textareaRef.current;
    if (el) {
      const selected = el.value.substring(el.selectionStart, el.selectionEnd);
      setSelectedText(selected);
      if (!selected) setRewriteResult(null);
    }
  };

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

  const applySuggestion = (issue: GrammarIssue, suggestion: string) => {
    const before = text.substring(0, issue.start);
    const after = text.substring(issue.end);
    const newText = before + suggestion + after;
    setText(newText);
    checkGrammar(newText);
  };

  const applyRewrite = () => {
    if (!rewriteResult?.rewritten) return;
    if (selectedText) {
      const start = text.indexOf(selectedText);
      if (start >= 0) {
        const newText = text.substring(0, start) + rewriteResult.rewritten + text.substring(start + selectedText.length);
        setText(newText);
        checkGrammar(newText);
      }
    } else {
      setText(rewriteResult.rewritten);
      checkGrammar(rewriteResult.rewritten);
    }
    setRewriteResult(null);
    setSelectedText("");
  };

  const getIssueSnippet = (issue: GrammarIssue): string => {
    return text.substring(issue.start, issue.end);
  };

  const issueSeverityClass = (severity: string): string => {
    const s = severity.toLowerCase();
    if (s.includes("error") || s.includes("spell")) return "error";
    if (s.includes("style") || s.includes("readability")) return "style";
    return "warning";
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

          <div className="editor-container">
            <textarea
              ref={textareaRef}
              className="editor-textarea"
              value={text}
              onChange={(e) => handleTextChange(e.target.value)}
              onSelect={handleSelect}
              placeholder={"Start writing, or paste your text here.\n\nGhostpen checks grammar instantly using Harper \u2014 everything stays on your machine.\n\nSelect text and click a rewrite button to get AI-powered suggestions from your local LLM (Ollama or LM Studio).\n\nNo cloud. No surveillance. No keystrokes logged. Just better writing."}
              spellCheck={false}
            />
          </div>
        </div>

        <div className="sidebar">
          <div className="sidebar-header">
            Issues {issues.length > 0 && `(${issues.length})`}
          </div>

          <div className="sidebar-content">
            {issues.length === 0 && text.trim() && (
              <div className="empty-state">
                <div className="empty-state-text">No issues found.</div>
              </div>
            )}

            {issues.length === 0 && !text.trim() && (
              <div className="empty-state">
                <div className="empty-state-text">
                  Type or paste text to check for grammar, spelling, and style issues.
                </div>
              </div>
            )}

            {issues.map((issue, i) => (
              <div key={i} className={`issue-card ${issueSeverityClass(issue.severity)}`}>
                <div className="issue-text">
                  <mark>{getIssueSnippet(issue)}</mark>
                </div>
                <div className="issue-message">{issue.message}</div>
                {issue.suggestions.length > 0 && (
                  <div className="issue-suggestions">
                    {issue.suggestions.map((s, j) => (
                      <button
                        key={j}
                        className="suggestion-chip"
                        onClick={() => applySuggestion(issue, s)}
                      >
                        {s}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>

          {(rewriteResult || rewriteLoading) && (
            <div className="rewrite-panel">
              <div style={{ fontSize: 14, fontWeight: 600, color: "#6d28d9" }}>
                {rewriteLoading ? (
                  <span><span className="spinner" /> Thinking...</span>
                ) : (
                  "Suggestion"
                )}
              </div>

              {rewriteResult && (
                <div className="rewrite-result">
                  {rewriteResult.rewritten && (
                    <div className="rewrite-text">{rewriteResult.rewritten}</div>
                  )}
                  {rewriteResult.explanation && (
                    <div className="rewrite-explanation">{rewriteResult.explanation}</div>
                  )}
                  {rewriteResult.rewritten && (
                    <div className="rewrite-actions">
                      <button className="btn-apply" onClick={applyRewrite}>Apply</button>
                      <button className="toolbar-btn" onClick={() => setRewriteResult(null)}>Dismiss</button>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <div className="footer">
        <span>Ghostpen v0.1.0 -- Your writing never leaves your machine</span>
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
