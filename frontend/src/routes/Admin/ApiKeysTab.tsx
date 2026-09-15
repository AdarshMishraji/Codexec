import { useEffect, useState } from "react";
import Modal from "../../components/Modal";
import { ApiError, adminCreateApiKey, adminDeleteApiKey, adminListApiKeys } from "../../lib/api";
import { fmtDate } from "../../lib/format";
import type { PublicApiKey } from "../../lib/types";

interface ApiKeysTabProps {
  token: string;
  onSessionExpired: () => void;
}

export default function ApiKeysTab({ token, onSessionExpired }: ApiKeysTabProps) {
  const [keys, setKeys] = useState<PublicApiKey[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const [createOpen, setCreateOpen] = useState(false);
  const [label, setLabel] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const [revealValue, setRevealValue] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  async function load() {
    try {
      setKeys(await adminListApiKeys(token));
      setListError(null);
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return onSessionExpired();
      setListError(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  async function removeKey(key: PublicApiKey) {
    if (!confirm(`Remove API key "${key.label}"? Any client using it will immediately lose access.`)) return;
    setBusyId(key.id);
    try {
      await adminDeleteApiKey(token, key.id);
      await load();
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return onSessionExpired();
      alert(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  }

  function openCreate() {
    setLabel("");
    setCreateError(null);
    setCreateOpen(true);
  }

  async function submitCreate() {
    const trimmed = label.trim();
    if (!trimmed) {
      setCreateError("Label is required.");
      return;
    }
    setCreating(true);
    try {
      const created = await adminCreateApiKey(token, trimmed);
      setCreateOpen(false);
      setRevealValue(created.api_key);
      setCopied(false);
      await load();
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return onSessionExpired();
      setCreateError(e instanceof Error ? e.message : String(e));
    } finally {
      setCreating(false);
    }
  }

  function copyReveal() {
    if (!revealValue) return;
    navigator.clipboard?.writeText(revealValue).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }

  return (
    <section>
      <div className="panel">
        <div className="toolbar">
          <h2>
            API keys <span className="count">{keys ? `(${keys.length})` : ""}</span>
          </h2>
          <button className="primary" onClick={openCreate}>
            + Create Key
          </button>
        </div>

        {listError && <div className="banner error">{listError}</div>}
        {!listError && keys && keys.length === 0 && (
          <div className="empty-state">No API keys yet — create one to authenticate submissions.</div>
        )}
        {!listError && keys && keys.length > 0 && (
          <div style={{ overflowX: "auto" }}>
            <table className="data">
              <thead>
                <tr>
                  <th>Label</th>
                  <th>Prefix</th>
                  <th>Status</th>
                  <th>Created</th>
                  <th>Last used</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {keys.map((k) => (
                  <tr key={k.id}>
                    <td>
                      <strong>{k.label}</strong>
                    </td>
                    <td className="mono">{k.key_prefix}…</td>
                    <td>
                      <span className={"badge " + (k.is_active ? "on" : "off")}>
                        {k.is_active ? "Active" : "Revoked"}
                      </span>
                    </td>
                    <td className="muted">{fmtDate(k.created_at)}</td>
                    <td className="muted">{fmtDate(k.last_used_at)}</td>
                    <td>
                      <button className="danger" disabled={busyId === k.id} onClick={() => removeKey(k)}>
                        Remove
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {createOpen && (
        <Modal
          title="Create API key"
          narrow
          onClose={() => setCreateOpen(false)}
          footer={
            <>
              <button className="link" onClick={() => setCreateOpen(false)}>
                Cancel
              </button>
              <button className="primary" disabled={creating} onClick={submitCreate}>
                {creating ? "Creating…" : "Create key"}
              </button>
            </>
          }
        >
          {createError && <div className="banner error">{createError}</div>}
          <label className="field">Label</label>
          <input
            type="text"
            placeholder="e.g. acme-corp-integration"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
          <div className="hint">A human-readable name so you can identify this key later.</div>
        </Modal>
      )}

      {revealValue && (
        <Modal
          title="Key created"
          narrow
          onClose={() => setRevealValue(null)}
          footer={
            <button className="primary" onClick={() => setRevealValue(null)}>
              Done
            </button>
          }
        >
          <div className="banner info">This key is shown only once. Copy it now — it can't be retrieved again.</div>
          <div className="reveal-box">
            <span>{revealValue}</span>
            <button className="copy-btn" onClick={copyReveal}>
              {copied ? "Copied!" : "Copy"}
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}
