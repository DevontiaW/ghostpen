import { useState, useEffect } from "react";
import Markdown from "react-markdown";

interface RewriteResult {
  rewritten: string;
  explanation: string;
}

interface RewritePanelProps {
  rewriteResult: RewriteResult | null;
  rewriteLoading: boolean;
  streamingText?: string;
  onApply: () => void;
  onDismiss: () => void;
  onCancel?: () => void;
  onFeedback: (rating: "good" | "bad", rewriteText: string, mode: string) => void;
  mode: string;
}

export default function RewritePanel({ rewriteResult, rewriteLoading, streamingText, onApply, onDismiss, onCancel, onFeedback, mode }: RewritePanelProps) {
  const [feedbackGiven, setFeedbackGiven] = useState(false);
  const [showThanks, setShowThanks] = useState(false);

  const handleFeedback = (rating: "good" | "bad") => {
    if (feedbackGiven || !rewriteResult?.rewritten) return;
    setFeedbackGiven(true);
    setShowThanks(true);
    onFeedback(rating, rewriteResult.rewritten, mode);
    setTimeout(() => setShowThanks(false), 1000);
  };

  useEffect(() => {
    if (!rewriteResult) {
      setFeedbackGiven(false);
      setShowThanks(false);
    }
  }, [rewriteResult]);

  if (!rewriteResult && !rewriteLoading) return null;

  const isCoachMode = mode === "explain";

  return (
    <div className="rewrite-panel">
      <div style={{ fontSize: 14, fontWeight: 600, color: "#6d28d9", display: "flex", alignItems: "center", gap: 8 }}>
        {rewriteLoading ? (
          <>
            <span><span className="spinner" /> Thinking...</span>
            {onCancel && (
              <button className="toolbar-btn" style={{ fontSize: 12, padding: "2px 8px" }} onClick={onCancel}>
                Cancel
              </button>
            )}
          </>
        ) : (
          isCoachMode ? "Writing Coach" : "Rewrite Suggestion"
        )}
      </div>

      {rewriteLoading && streamingText && (
        <div className="rewrite-result">
          <div className="rewrite-text" style={{ opacity: 0.8 }}>
            <Markdown>{streamingText}</Markdown>
          </div>
        </div>
      )}

      {rewriteResult && (
        <div className="rewrite-result">
          {rewriteResult.rewritten && !isCoachMode && (
            <>
              <div style={{ fontSize: 11, fontWeight: 600, textTransform: "uppercase", color: "#7c3aed", letterSpacing: "0.5px", marginBottom: 4 }}>
                Suggested rewrite
              </div>
              <div className="rewrite-text">
                <Markdown>{rewriteResult.rewritten}</Markdown>
              </div>
            </>
          )}
          {rewriteResult.rewritten && isCoachMode && (
            <div className="rewrite-text">
              <Markdown>{rewriteResult.rewritten}</Markdown>
            </div>
          )}
          {rewriteResult.explanation && (
            <>
              {!isCoachMode && (
                <div style={{ fontSize: 11, fontWeight: 600, textTransform: "uppercase", color: "#6b7280", letterSpacing: "0.5px", marginBottom: 4, marginTop: 8 }}>
                  Why this is better
                </div>
              )}
              <div className="rewrite-explanation">
                <Markdown>{rewriteResult.explanation}</Markdown>
              </div>
            </>
          )}
          {(rewriteResult.rewritten || rewriteResult.explanation) && (
            <div className="rewrite-actions">
              {rewriteResult.rewritten && !isCoachMode && (
                <button className="btn-apply" onClick={onApply}>Apply Rewrite</button>
              )}
              <button className="toolbar-btn" onClick={onDismiss}>Dismiss</button>
              <div className="feedback-group">
                <button
                  className={`feedback-btn ${feedbackGiven ? "disabled" : ""}`}
                  onClick={() => handleFeedback("good")}
                  disabled={feedbackGiven}
                  title="Helpful"
                >
                  Good
                </button>
                <button
                  className={`feedback-btn ${feedbackGiven ? "disabled" : ""}`}
                  onClick={() => handleFeedback("bad")}
                  disabled={feedbackGiven}
                  title="Not helpful"
                >
                  Bad
                </button>
                {showThanks && <span className="feedback-thanks">Thanks!</span>}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export type { RewriteResult };
