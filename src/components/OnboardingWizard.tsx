import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

interface LlmStatus {
  available: boolean;
  provider: string;
  model: string;
}

interface OnboardingWizardProps {
  onComplete: () => void;
}

export default function OnboardingWizard({ onComplete }: OnboardingWizardProps) {
  const [step, setStep] = useState(0);
  const [llmStatus, setLlmStatus] = useState<LlmStatus | null>(null);
  const [checking, setChecking] = useState(false);

  const checkLlm = async () => {
    setChecking(true);
    try {
      const status = await invoke<LlmStatus>("check_llm_status");
      setLlmStatus(status);
    } catch {
      setLlmStatus({ available: false, provider: "none", model: "" });
    }
    setChecking(false);
  };

  useEffect(() => {
    if (step === 1) {
      checkLlm();
    }
  }, [step]);

  const handleComplete = () => {
    localStorage.setItem("ghostpen-onboarded", "true");
    onComplete();
  };

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-card">
        <div className="onboarding-dots">
          {[0, 1, 2].map((i) => (
            <div key={i} className={`onboarding-dot ${step === i ? "active" : step > i ? "done" : ""}`} />
          ))}
        </div>

        {step === 0 && (
          <div className="onboarding-step">
            <h2>Welcome to Ghost<span style={{ color: "#7c3aed" }}>pen</span></h2>
            <p className="onboarding-lead">
              Your writing never leaves your machine.
            </p>
            <p>
              Ghostpen checks grammar instantly using Harper (no network needed) and offers
              AI-powered rewrites through your local LLM. No cloud. No surveillance. No
              keystrokes logged.
            </p>
            <button className="onboarding-btn" onClick={() => setStep(1)}>
              Next
            </button>
          </div>
        )}

        {step === 1 && (
          <div className="onboarding-step">
            <h2>AI Setup</h2>
            <p>
              For AI-powered rewrites and coaching, Ghostpen connects to a local LLM server
              on your machine.
            </p>

            <div className="onboarding-llm-status">
              {checking ? (
                <div className="onboarding-checking">
                  <span className="spinner" /> Checking for local LLM...
                </div>
              ) : llmStatus?.available ? (
                <div className="onboarding-found">
                  <div className="onboarding-found-badge">Ready</div>
                  <strong>{llmStatus.provider}</strong> detected
                  {llmStatus.model && llmStatus.model !== "default" && (
                    <span> with model <code>{llmStatus.model}</code></span>
                  )}
                </div>
              ) : (
                <div className="onboarding-not-found">
                  <p>No local LLM detected. For AI rewrites, install one of these:</p>
                  <div className="onboarding-links">
                    <button className="onboarding-link-btn" onClick={() => openUrl("https://ollama.ai")}>
                      Ollama (recommended)
                    </button>
                    <button className="onboarding-link-btn" onClick={() => openUrl("https://lmstudio.ai")}>
                      LM Studio
                    </button>
                  </div>
                  <p className="onboarding-note">
                    Grammar checking works without this. You can set up AI later.
                  </p>
                  <button className="onboarding-recheck" onClick={checkLlm}>
                    Re-check
                  </button>
                </div>
              )}
            </div>

            <button className="onboarding-btn" onClick={() => setStep(2)}>
              {llmStatus?.available ? "Next" : "Skip for now"}
            </button>
          </div>
        )}

        {step === 2 && (
          <div className="onboarding-step">
            <h2>You're all set</h2>
            <p>Start writing. Ghostpen will check your grammar as you type.</p>
            <div className="onboarding-tips">
              <div className="onboarding-tip"><strong>Ctrl+O</strong> Open a file</div>
              <div className="onboarding-tip"><strong>Ctrl+S</strong> Save</div>
              <div className="onboarding-tip"><strong>Ctrl+.</strong> Quick-fix at cursor</div>
              <div className="onboarding-tip"><strong>Select text</strong> then click a rewrite mode</div>
            </div>
            <button className="onboarding-btn" onClick={handleComplete}>
              Start Writing
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
