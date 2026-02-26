interface RewriteResult {
  rewritten: string;
  explanation: string;
}

interface RewritePanelProps {
  rewriteResult: RewriteResult | null;
  rewriteLoading: boolean;
  onApply: () => void;
  onDismiss: () => void;
}

export default function RewritePanel({ rewriteResult, rewriteLoading, onApply, onDismiss }: RewritePanelProps) {
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
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export type { RewriteResult };
