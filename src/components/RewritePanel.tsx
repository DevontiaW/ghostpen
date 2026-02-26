import { useState } from "react";

interface RewriteResult {
  rewritten: string;
  explanation: string;
}

interface RewritePanelProps {
  rewriteResult: RewriteResult | null;
  rewriteLoading: boolean;
  onApply: () => void;
  onDismiss: () => void;
  onFeedback: (rating: "good" | "bad", rewriteText: string, mode: string) => void;
  mode: string;
}

export default function RewritePanel({ rewriteResult, rewriteLoading, onApply, onDismiss, onFeedback, mode }: RewritePanelProps) {
  const [feedbackGiven, setFeedbackGiven] = useState(false);
  const [showThanks, setShowThanks] = useState(false);

  // Reset feedback state when rewrite result changes
  const handleFeedback = (rating: "good" | "bad") => {
    if (feedbackGiven || !rewriteResult?.rewritten) return;
    setFeedbackGiven(true);
    setShowThanks(true);
    onFeedback(rating, rewriteResult.rewritten, mode);
    setTimeout(() => setShowThanks(false), 1000);
  };

  // Reset when new result arrives
  if (!rewriteResult && feedbackGiven) {
    setFeedbackGiven(false);
    setShowThanks(false);
  }

  if (!rewriteResult && !rewriteLoading) return null;

  return (
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
              <button className="btn-apply" onClick={onApply}>Apply</button>
              <button className="toolbar-btn" onClick={onDismiss}>Dismiss</button>
              <div className="feedback-group">
                <button
                  className={`feedback-btn ${feedbackGiven ? "disabled" : ""}`}
                  onClick={() => handleFeedback("good")}
                  disabled={feedbackGiven}
                  title="Good rewrite"
                >
                  Good
                </button>
                <button
                  className={`feedback-btn ${feedbackGiven ? "disabled" : ""}`}
                  onClick={() => handleFeedback("bad")}
                  disabled={feedbackGiven}
                  title="Bad rewrite"
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
