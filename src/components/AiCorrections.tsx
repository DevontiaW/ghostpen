import { memo } from "react";
import type { TextChange, AiCorrectionResult } from "../types/ai";

interface Props {
  result: AiCorrectionResult | null;
  loading: boolean;
  onAcceptAll: () => void;
  onAcceptChange: (change: TextChange) => void;
  onDismiss: () => void;
}

export default memo(function AiCorrections({
  result,
  loading,
  onAcceptAll,
  onAcceptChange,
  onDismiss,
}: Props) {
  if (!loading && !result) return null;

  if (loading) {
    return (
      <div className="ai-corrections-panel">
        <div className="ai-loading">
          <span className="spinner" />
          Checking grammar...
        </div>
      </div>
    );
  }

  if (result && result.changes.length === 0) {
    return (
      <div className="ai-corrections-panel">
        <div className="ai-no-changes">No corrections needed</div>
      </div>
    );
  }

  return (
    <div className="ai-corrections-panel">
      <div className="ai-corrections-header">
        <span className="ai-corrections-title">
          AI Corrections
          <span className="ai-corrections-count">
            ({result!.changes.length} change{result!.changes.length !== 1 ? "s" : ""})
          </span>
        </span>
        <div className="ai-corrections-actions">
          <button className="btn-accept-all" onClick={onAcceptAll}>
            Accept All
          </button>
          <button className="btn-dismiss-ai" onClick={onDismiss}>
            Dismiss
          </button>
        </div>
      </div>

      {result!.changes.map((change, i) => (
        <div key={`${change.start}-${change.end}-${i}`} className="ai-change-card">
          <div className="ai-change-text">
            {change.original ? (
              <>
                <span className="ai-change-removed">{change.original}</span>
                <span className="ai-change-arrow">&rarr;</span>
              </>
            ) : (
              <span className="ai-change-insert-label">insert:</span>
            )}
            <span className="ai-change-added">{change.replacement}</span>
          </div>
          <button
            className="ai-change-accept"
            onClick={() => onAcceptChange(change)}
          >
            Accept
          </button>
        </div>
      ))}
    </div>
  );
});
