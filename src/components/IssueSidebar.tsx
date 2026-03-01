import { invoke } from "@tauri-apps/api/core";
import type { GrammarIssue } from "./Editor";

interface IssueSidebarProps {
  issues: GrammarIssue[];
  text: string;
  onApplySuggestion: (issue: GrammarIssue, suggestion: string) => void;
  onScrollToIssue: (issue: GrammarIssue) => void;
  onDictionaryAdd?: () => void;
}

function issueSeverityClass(severity: string): string {
  const s = severity.toLowerCase();
  if (s.includes("error") || s.includes("spell")) return "error";
  if (s.includes("style") || s.includes("readability")) return "style";
  return "warning";
}

export default function IssueSidebar({ issues, text, onApplySuggestion, onScrollToIssue, onDictionaryAdd }: IssueSidebarProps) {
  const handleAddToDictionary = async (issue: GrammarIssue) => {
    const word = text.substring(issue.start, issue.end).trim();
    if (!word) return;
    try {
      await invoke("add_to_dictionary", { word });
      onDictionaryAdd?.();
    } catch (err) {
      console.error("Failed to add to dictionary:", err);
    }
  };

  const isSpellingIssue = (severity: string): boolean => {
    const s = severity.toLowerCase();
    return s.includes("spell") || s.includes("error");
  };
  const getIssueSnippet = (issue: GrammarIssue): string => {
    return text.substring(issue.start, issue.end);
  };

  return (
    <>
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
          <div
            key={i}
            className={`issue-card ${issueSeverityClass(issue.severity)}`}
            onClick={() => onScrollToIssue(issue)}
          >
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
                    onClick={(e) => {
                      e.stopPropagation();
                      onApplySuggestion(issue, s);
                    }}
                  >
                    {s}
                  </button>
                ))}
                {isSpellingIssue(issue.severity) && (
                  <button
                    className="suggestion-chip dict-chip"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleAddToDictionary(issue);
                    }}
                  >
                    + Dictionary
                  </button>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
    </>
  );
}
