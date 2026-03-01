import { useState, useEffect, useCallback, useRef, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";
import { isolateHistory } from "@codemirror/commands";
import Editor from "./components/Editor";
import { highlightEffect } from "./components/Editor";
import IssueSidebar from "./components/IssueSidebar";
import AiCorrections from "./components/AiCorrections";
const RewritePanel = lazy(() => import("./components/RewritePanel"));
import OnboardingWizard from "./components/OnboardingWizard";
import type { GrammarIssue, CheckResult } from "./components/Editor";
// Type-only import doesn't affect code splitting
import type { RewriteResult } from "./components/RewritePanel";
import { logEvent } from "./logger";
import "./App.css";

interface LlmStatus {
  available: boolean;
  provider: string;
  model: string;
}

interface TextChange {
  start: number;
  end: number;
  original: string;
  replacement: string;
}

interface AiCorrectionResult {
  original: string;
  corrected: string;
  changes: TextChange[];
}

const DRAFT_KEY = "ghostpen-draft";
const ONBOARDED_KEY = "ghostpen-onboarded";
const RECENT_FILES_KEY = "ghostpen-recent-files";
const WRITING_STATS_KEY = "ghostpen-writing-stats";

interface RecentFile {
  path: string;
  name: string;
}

function safeParseJSON<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : fallback;
  } catch {
    return fallback;
  }
}

function addToRecentFiles(path: string): RecentFile[] {
  const name = path.split(/[/\\]/).pop() || path;
  const existing: RecentFile[] = safeParseJSON(RECENT_FILES_KEY, []);
  const filtered = existing.filter(f => f.path !== path);
  const updated = [{ path, name }, ...filtered].slice(0, 5);
  localStorage.setItem(RECENT_FILES_KEY, JSON.stringify(updated));
  return updated;
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
  const [llmLaunching, setLlmLaunching] = useState(false);
  const [lastUsedMode, setLastUsedMode] = useState<string>("clarity");
  const [currentFilePath, setCurrentFilePath] = useState<string | null>(null);
  const [isDirty, setIsDirty] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  const [recentFiles, setRecentFiles] = useState<RecentFile[]>(() =>
    safeParseJSON<RecentFile[]>(RECENT_FILES_KEY, [])
  );
  const [showRecentFiles, setShowRecentFiles] = useState(false);
  const [wordsToday, setWordsToday] = useState(0);
  const lastWordCountRef = useRef(0);
  const editorRef = useRef<ReactCodeMirrorRef | null>(null);
  const highlightTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const textChangeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [aiResult, setAiResult] = useState<AiCorrectionResult | null>(null);
  const [aiLoading, setAiLoading] = useState(false);

  // Show toast briefly
  const showToast = (msg: string) => {
    setToastMessage(msg);
    setTimeout(() => setToastMessage(null), 2000);
  };

  // --- File operations ---
  const handleNew = useCallback(() => {
    setText("");
    setCurrentFilePath(null);
    setIsDirty(false);
    setIssues([]);
    setRewriteResult(null);
    lastWordCountRef.current = 0;
    localStorage.removeItem(DRAFT_KEY);
    logEvent("new_document");
  }, []);

  const handleOpen = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: "Text Files", extensions: ["txt", "md", "markdown"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (!selected) return;
      const path = typeof selected === "string" ? selected : selected;
      const content = await readTextFile(path);
      setText(content);
      setCurrentFilePath(path);
      setIsDirty(false);
      lastWordCountRef.current = content.split(/\s+/).filter(Boolean).length;
      localStorage.removeItem(DRAFT_KEY);
      setRecentFiles(addToRecentFiles(path));
      logEvent("file_opened", { path });
    } catch (err) {
      console.error("Failed to open file:", err);
    }
  }, []);

  const handleSaveAs = useCallback(async () => {
    try {
      const path = await save({
        filters: [
          { name: "Text Files", extensions: ["txt", "md"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (!path) return;
      await writeTextFile(path, text);
      setCurrentFilePath(path);
      setIsDirty(false);
      showToast("Saved");
      setRecentFiles(addToRecentFiles(path));
      logEvent("file_saved_as", { path });
    } catch (err) {
      console.error("Failed to save file:", err);
    }
  }, [text]);

  const handleSave = useCallback(async () => {
    if (currentFilePath) {
      try {
        await writeTextFile(currentFilePath, text);
        setIsDirty(false);
        showToast("Saved");
        logEvent("file_saved", { path: currentFilePath });
      } catch (err) {
        console.error("Failed to save file:", err);
      }
    } else {
      handleSaveAs();
    }
  }, [currentFilePath, text, handleSaveAs]);

  const openRecentFile = useCallback(async (path: string) => {
    try {
      const content = await readTextFile(path);
      setText(content);
      setCurrentFilePath(path);
      setIsDirty(false);
      lastWordCountRef.current = content.split(/\s+/).filter(Boolean).length;
      localStorage.removeItem(DRAFT_KEY);
      setRecentFiles(addToRecentFiles(path));
      setShowRecentFiles(false);
      logEvent("file_opened_recent", { path });
    } catch (err) {
      console.error("Failed to open recent file:", err);
      showToast("Failed to open file");
    }
  }, []);

  const handleCopyAll = useCallback(async () => {
    try {
      await writeText(text);
      showToast("Copied!");
      logEvent("text_copied", { length: text.length });
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }, [text]);

  const handleExport = useCallback(async () => {
    try {
      const path = await save({
        filters: [
          { name: "Text", extensions: ["txt"] },
          { name: "Markdown", extensions: ["md"] },
        ],
      });
      if (!path) return;
      await writeTextFile(path, text);
      showToast("Exported");
      logEvent("file_exported", { path });
    } catch (err) {
      console.error("Failed to export:", err);
    }
  }, [text]);

  // Expose file handlers for Editor keybindings
  const fileHandlersRef = useRef({ handleNew, handleOpen, handleSave, handleSaveAs });
  useEffect(() => {
    fileHandlersRef.current = { handleNew, handleOpen, handleSave, handleSaveAs };
  }, [handleNew, handleOpen, handleSave, handleSaveAs]);

  // --- Onboarding check ---
  useEffect(() => {
    if (!localStorage.getItem(ONBOARDED_KEY)) {
      setShowOnboarding(true);
    }
  }, []);

  // --- Click-outside to dismiss recent files dropdown ---
  useEffect(() => {
    if (!showRecentFiles) return;
    const handler = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest('.file-name-group')) {
        setShowRecentFiles(false);
      }
    };
    document.addEventListener('click', handler);
    return () => document.removeEventListener('click', handler);
  }, [showRecentFiles]);

  // --- Writing stats: load today's count on mount ---
  useEffect(() => {
    const today = new Date().toISOString().slice(0, 10);
    const stats = safeParseJSON<Record<string, number>>(WRITING_STATS_KEY, {});
    setWordsToday(stats[today] || 0);
  }, []);

  // --- Restore draft from localStorage on mount ---
  useEffect(() => {
    const savedDraft = localStorage.getItem(DRAFT_KEY);
    if (savedDraft && !currentFilePath) {
      setText(savedDraft);
      lastWordCountRef.current = savedDraft.split(/\s+/).filter(Boolean).length;
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const scrollToIssue = useCallback((issue: GrammarIssue) => {
    const view = editorRef.current?.view;
    if (!view) return;

    if (highlightTimerRef.current) {
      clearTimeout(highlightTimerRef.current);
    }

    view.dispatch({
      effects: EditorView.scrollIntoView(
        EditorSelection.range(issue.start, issue.end),
        { y: "center" }
      ),
    });

    view.dispatch({
      effects: highlightEffect.of({ from: issue.start, to: issue.end }),
    });

    highlightTimerRef.current = setTimeout(() => {
      if (editorRef.current?.view) {
        editorRef.current.view.dispatch({
          effects: highlightEffect.of(null),
        });
      }
      highlightTimerRef.current = null;
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
    if (newIssues.length > 0) {
      logEvent("issues_found", { count: newIssues.length });
    }
  }, []);

  const handleSelectionChange = useCallback((selected: string, from: number, to: number) => {
    setSelectedText(selected);
    setSelectionRange(selected ? { from, to } : null);
    if (!selected) setRewriteResult(null);
  }, []);

  const [streamingText, setStreamingText] = useState("");
  const isRewriting = useRef(false);
  const rewriteSelectionRef = useRef<{ from: number; to: number } | null>(null);

  const handleRewrite = async (mode: string) => {
    if (isRewriting.current) return;
    const targetText = selectedText || text;
    if (!targetText.trim()) return;
    if (targetText.length > 5000) {
      setRewriteResult({
        rewritten: "",
        explanation: "Text too long for rewrite (max 5,000 characters). Select a smaller portion of text and try again.",
      });
      return;
    }

    isRewriting.current = true;
    rewriteSelectionRef.current = selectionRange;
    setActiveMode(mode);
    setLastUsedMode(mode);
    setRewriteLoading(true);
    setRewriteResult(null);
    setStreamingText("");
    logEvent("rewrite_requested", { mode, text_length: targetText.length });

    const unlisten = await listen<string>("rewrite-stream", (event) => {
      setStreamingText(event.payload);
    });

    try {
      const result = await invoke<RewriteResult>("rewrite_text_stream", {
        request: { text: targetText, mode },
      });
      setRewriteResult(result);
      setStreamingText("");
    } catch (err) {
      const errStr = String(err);
      if (!errStr.includes("cancelled")) {
        setRewriteResult({
          rewritten: "",
          explanation: `Error: ${err}. Make sure Ollama or LM Studio is running locally.`,
        });
      }
      setStreamingText("");
    } finally {
      unlisten();
      isRewriting.current = false;
      setRewriteLoading(false);
      setActiveMode(null);
    }
  };

  const handleCancelRewrite = async () => {
    try {
      await invoke("cancel_rewrite");
    } catch { /* ignore */ }
  };

  const handleAiCheck = async () => {
    if (aiLoading || !text.trim()) return;
    setAiLoading(true);
    setAiResult(null);
    logEvent("ai_check_requested", { text_length: text.length });
    try {
      const result = await invoke<AiCorrectionResult>("correct_grammar_ai", { text });
      setAiResult(result);
    } catch (err) {
      setAiResult({
        original: text,
        corrected: text,
        changes: [],
      });
      console.error("AI check failed:", err);
    } finally {
      setAiLoading(false);
    }
  };

  const handleAcceptAllAi = () => {
    if (!aiResult?.corrected) return;
    const view = editorRef.current?.view;
    if (!view) return;
    const annotation = isolateHistory.of("full");
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: aiResult.corrected },
      annotations: annotation,
    });
    setAiResult(null);
    logEvent("ai_corrections_accepted_all");
  };

  const handleAcceptAiChange = (change: TextChange) => {
    const view = editorRef.current?.view;
    if (!view) return;
    view.dispatch({
      changes: { from: change.start, to: change.end, insert: change.replacement },
    });
    if (aiResult) {
      const lengthDiff = change.replacement.length - (change.end - change.start);
      const updatedChanges = aiResult.changes
        .filter(c => c.start !== change.start || c.end !== change.end)
        .map(c => {
          if (c.start > change.start) {
            return { ...c, start: c.start + lengthDiff, end: c.end + lengthDiff };
          }
          return c;
        });
      if (updatedChanges.length === 0) {
        setAiResult(null);
      } else {
        setAiResult({ ...aiResult, changes: updatedChanges });
      }
    }
    logEvent("ai_correction_accepted_single");
  };

  const handleQuickFix = useCallback((pos: number) => {
    const issue = issues.find(i => pos >= i.start && pos <= i.end);
    if (!issue || issue.suggestions.length === 0) return;
    logEvent("quick_fix", { position: pos });
    const view = editorRef.current?.view;
    if (!view) return;
    view.dispatch({ changes: { from: issue.start, to: issue.end, insert: issue.suggestions[0] } });
    setIssues([]);
  }, [issues]);

  const applySuggestion = (issue: GrammarIssue, suggestion: string) => {
    logEvent("suggestion_applied", { issue_message: issue.message });
    const view = editorRef.current?.view;
    if (!view) return;
    view.dispatch({ changes: { from: issue.start, to: issue.end, insert: suggestion } });
    setIssues([]);
  };

  const applyRewrite = () => {
    if (!rewriteResult?.rewritten) return;
    const view = editorRef.current?.view;
    if (!view) return;
    const capturedRange = rewriteSelectionRef.current;
    const annotation = isolateHistory.of("full");
    if (capturedRange) {
      view.dispatch({
        changes: { from: capturedRange.from, to: capturedRange.to, insert: rewriteResult.rewritten },
        annotations: annotation,
      });
    } else {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: rewriteResult.rewritten },
        annotations: annotation,
      });
    }
    setRewriteResult(null);
    setSelectedText("");
    setSelectionRange(null);
  };

  const handleFeedback = async (rating: "good" | "bad", rewriteText: string, mode: string) => {
    logEvent("rewrite_feedback", { rating, mode });
    try {
      await invoke("save_feedback", {
        feedback: {
          rating,
          original_text: selectedText || text,
          rewritten_text: rewriteText,
          mode,
        },
      });
    } catch (err) {
      console.error("Failed to save feedback:", err);
    }
  };

  const handleTextChange = (newText: string) => {
    setText(newText);
    setIsDirty(true);

    // Track writing stats — count words added
    const newWordCount = newText.split(/\s+/).filter(Boolean).length;
    const delta = newWordCount - lastWordCountRef.current;
    lastWordCountRef.current = newWordCount;
    if (delta > 0) {
      const today = new Date().toISOString().slice(0, 10);
      const stats = safeParseJSON<Record<string, number>>(WRITING_STATS_KEY, {});
      stats[today] = (stats[today] || 0) + delta;
      // Prune entries older than 30 days
      const cutoff = new Date();
      cutoff.setDate(cutoff.getDate() - 30);
      const cutoffStr = cutoff.toISOString().slice(0, 10);
      for (const key of Object.keys(stats)) {
        if (key < cutoffStr) delete stats[key];
      }
      localStorage.setItem(WRITING_STATS_KEY, JSON.stringify(stats));
      setWordsToday(stats[today]);
    }

    // Debounced auto-save to localStorage (1s)
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
    }
    autoSaveTimerRef.current = setTimeout(() => {
      localStorage.setItem(DRAFT_KEY, newText);
      autoSaveTimerRef.current = null;
    }, 1000);

    // Debounced text change logging
    if (textChangeTimerRef.current) {
      clearTimeout(textChangeTimerRef.current);
    }
    textChangeTimerRef.current = setTimeout(() => {
      const word_count = newText.split(/\s+/).filter(Boolean).length;
      logEvent("text_change", { word_count });
      textChangeTimerRef.current = null;
    }, 2000);
  };

  // Extract display filename from path
  const displayName = currentFilePath
    ? currentFilePath.split(/[/\\]/).pop() || "Untitled"
    : "Untitled";

  return (
    <div className="app">
      {showOnboarding && (
        <OnboardingWizard onComplete={() => setShowOnboarding(false)} />
      )}

      <div className="header">
        <div className="header-left">
          <div className="logo">Ghost<span>pen</span></div>
          <div className="file-name-group">
            <div className="file-name" onClick={() => recentFiles.length > 0 && setShowRecentFiles(!showRecentFiles)}>
              {displayName}{isDirty && <span className="dirty-dot" title="Unsaved changes" />}
              {recentFiles.length > 0 && <span className="recent-arrow">{showRecentFiles ? "\u25B4" : "\u25BE"}</span>}
            </div>
            {showRecentFiles && recentFiles.length > 0 && (
              <div className="recent-files-dropdown">
                {recentFiles.map((f, i) => (
                  <div key={i} className="recent-file-item" onClick={() => openRecentFile(f.path)}>
                    <span className="recent-file-name">{f.name}</span>
                    <span className="recent-file-path">{f.path}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
          <div className={`status-badge ${llmStatus.available ? "connected" : "disconnected"}`}>
            <div className="status-dot" />
            {llmStatus.available ? `${llmStatus.provider}` : "No LLM"}
          </div>
          {!llmStatus.available && !llmLaunching && (
            <button
              className="launch-llm-btn"
              onClick={async () => {
                setLlmLaunching(true);
                logEvent("llm_launch_requested");
                try {
                  await invoke<string>("launch_llm");
                } catch (e) {
                  console.error("Failed to launch LLM:", e);
                  setLlmLaunching(false);
                  return;
                }
                let attempts = 0;
                const poll = setInterval(async () => {
                  attempts++;
                  try {
                    const status = await invoke<LlmStatus>("check_llm_status");
                    if (status.available) {
                      setLlmStatus(status);
                      setLlmLaunching(false);
                      clearInterval(poll);
                    }
                  } catch { /* ignore */ }
                  if (attempts >= 15) {
                    setLlmLaunching(false);
                    clearInterval(poll);
                  }
                }, 2000);
              }}
            >
              Launch LM Studio
            </button>
          )}
          {llmLaunching && (
            <span className="llm-launching">
              <span className="spinner" /> Starting LLM...
            </span>
          )}
        </div>
        <div className="header-right">
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
          <button
            className="help-btn"
            onClick={() => setShowOnboarding(true)}
            title="Show help"
          >
            ?
          </button>
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
            <div className="toolbar-separator" />
            <button
              className={`toolbar-btn ai-check-btn ${aiLoading ? "active" : ""}`}
              onClick={handleAiCheck}
              disabled={aiLoading || !text.trim()}
              title="AI grammar check (no server needed)"
            >
              {aiLoading && <span className="spinner" />}
              {" "}AI Check
            </button>
            <div className="toolbar-separator" />
            <button className="toolbar-btn toolbar-btn-icon" onClick={handleCopyAll} title="Copy all text">
              Copy
            </button>
            <button className="toolbar-btn toolbar-btn-icon" onClick={handleExport} title="Export to file">
              Export
            </button>
            {selectedText && (
              <span style={{ fontSize: 12, color: "#7c3aed", marginLeft: 8 }}>
                {selectedText.length} chars selected
              </span>
            )}
          </div>

          <Editor
            value={text}
            onChange={handleTextChange}
            onIssuesFound={handleIssuesFound}
            onSelectionChange={handleSelectionChange}
            editorRef={editorRef}
            onQuickFix={handleQuickFix}
            fileHandlersRef={fileHandlersRef}
          />
        </div>

        <div className="sidebar">
          <IssueSidebar
            issues={issues}
            text={text}
            onApplySuggestion={applySuggestion}
            onScrollToIssue={scrollToIssue}
            onDictionaryAdd={() => {
              // Force grammar re-check by requesting lint recalculation
              const view = editorRef.current?.view;
              if (view) {
                // Dispatch a no-content transaction to trigger linter re-run
                // The linter fires on any transaction, even empty ones
                view.dispatch({});
              }
            }}
          />

          <AiCorrections
            result={aiResult}
            loading={aiLoading}
            onAcceptAll={handleAcceptAllAi}
            onAcceptChange={handleAcceptAiChange}
            onDismiss={() => setAiResult(null)}
          />

          <Suspense fallback={null}>
            <RewritePanel
              rewriteResult={rewriteResult}
              rewriteLoading={rewriteLoading}
              streamingText={streamingText}
              onApply={applyRewrite}
              onDismiss={() => setRewriteResult(null)}
              onCancel={handleCancelRewrite}
              onFeedback={handleFeedback}
              mode={lastUsedMode}
            />
          </Suspense>
        </div>
      </div>

      <div className="footer">
        <span>Ghostpen v0.5.0 -- Your writing never leaves your machine</span>
        <span className="footer-stats">
          {wordsToday > 0 && <span className="words-today">{wordsToday} words today</span>}
          <span>
            {llmStatus.available
              ? `${llmStatus.provider} (${llmStatus.model})`
              : "Install Ollama or LM Studio for AI rewrites"
            }
          </span>
        </span>
      </div>

      {toastMessage && (
        <div className="toast">{toastMessage}</div>
      )}
    </div>
  );
}

export default App;
