import { CheckCircle2, ExternalLink, KeyRound, LockKeyhole, ShieldCheck, X } from "lucide-react";
import { useState } from "react";
import "./ExternalAccountPanel.css";

type ExternalAccountPanelProps = {
  onClose: () => void;
  onOpenPortal: () => Promise<void>;
  embedded?: boolean;
};

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL as string | undefined;
const accountPortalUrl = import.meta.env.VITE_FUNG_WEB_APP_URL as string | undefined;

/**
 * A browser-safe account surface. Authentication is intentionally not started
 * until runtime configuration and the OAuth callback implementation exist.
 */
export function ExternalAccountPanel({ onClose, onOpenPortal, embedded }: ExternalAccountPanelProps) {
  const [portalError, setPortalError] = useState<string | null>(null);
  const configured = Boolean(supabaseUrl && accountPortalUrl?.startsWith("https://"));

  const openPortal = async () => {
    setPortalError(null);
    try {
      await onOpenPortal();
    } catch (error) {
      setPortalError(error instanceof Error ? error.message : "The hosted account portal could not be opened.");
    }
  };

  const content = (
    <section
      className="account-panel"
      aria-label="Account and external connections"
      aria-modal="true"
      role="dialog"
      onMouseDown={(event) => event.stopPropagation()}
    >
      <header className="account-panel__header">
        <div>
          <span className="account-panel__eyebrow">Account and connections</span>
          <h1>Keep FUNG local. Connect only when you choose.</h1>
        </div>
        <button className="account-icon-button" type="button" onClick={onClose} aria-label="Close account panel">
          <X size={18} />
        </button>
      </header>

      <div className="account-panel__body">
        <section className="account-local-state" aria-labelledby="local-state-title">
          <ShieldCheck size={22} aria-hidden="true" />
          <div>
            <h2 id="local-state-title">Local workspace stays available</h2>
            <p>Capture, transcripts, notes, and GenesisBlockDB remain on this device. Signing in is never required to use FUNG.</p>
          </div>
        </section>

        <section className="account-connection" aria-labelledby="connection-title">
          <div className="account-connection__title">
            <span className={`account-status ${configured ? "account-status--ready" : "account-status--pending"}`}>
              {configured ? "Ready to connect" : "Setup pending"}
            </span>
            <h2 id="connection-title">External account</h2>
            <p>Optional Supabase identity for account settings and future opt-in sync metadata. No project content is uploaded by this screen.</p>
          </div>

          {configured ? (
            <div className="account-action-area">
              <p className="account-hint"><CheckCircle2 size={15} /> Secure auth service is configured for this build.</p>
              <button className="account-button account-button--primary" type="button" onClick={() => void openPortal()}>
                <KeyRound size={16} /> Open secure account portal
              </button>
            </div>
          ) : (
            <div className="account-action-area">
              <p className="account-hint"><LockKeyhole size={15} /> This browser preview has no auth endpoint configured.</p>
              <button className="account-button" type="button" onClick={() => void openPortal()}>
                <KeyRound size={16} /> Open account portal
              </button>
            </div>
          )}
          {portalError && <p className="account-error" role="alert">{portalError}</p>}
        </section>

        <section className="account-safeguards" aria-label="Connection safeguards">
          <div><strong>Minimal access</strong><span>Only explicitly approved scopes can be requested.</span></div>
          <div><strong>Short-lived access</strong><span>Access tokens are validated and never displayed here.</span></div>
          <div><strong>Always reversible</strong><span>You can revoke access and remove local credentials.</span></div>
        </section>

        <button className="account-revoke" type="button" disabled title="Hosted revocation is enabled after the provider callback is deployed.">
          Manage or revoke hosted connection
        </button>
      </div>

      <footer className="account-panel__footer">
        <span><LockKeyhole size={14} /> Credentials belong in secure storage, never in the project database.</span>
        <a href="https://supabase.com/docs/guides/auth" target="_blank" rel="noreferrer">How account connections work <ExternalLink size={13} /></a>
      </footer>
    </section>
  );

  if (embedded) return content;

  return (
    <div className="account-overlay" role="presentation" onMouseDown={onClose}>
      {content}
    </div>
  );
}
