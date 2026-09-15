import { useState } from "react";

interface AdminGateProps {
  error: string | null;
  onUnlock: (token: string) => void | Promise<void>;
}

export default function AdminGate({ error, onUnlock }: AdminGateProps) {
  const [candidate, setCandidate] = useState("");

  function submit() {
    const trimmed = candidate.trim();
    if (trimmed) onUnlock(trimmed);
  }

  return (
    <div id="gate">
      <div className="gate-card">
        <img className="brand-mark" src="/icon.png" alt="" style={{ width: 46, height: 46, margin: "0 auto 16px" }} />
        <h2>Admin Portal</h2>
        <p>Enter the admin API token to continue.</p>
        {error && <div className="banner error">{error}</div>}
        <input
          type="password"
          placeholder="Admin token"
          autoComplete="off"
          value={candidate}
          onChange={(e) => setCandidate(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
          }}
        />
        <button className="primary" onClick={submit}>
          Unlock
        </button>
      </div>
    </div>
  );
}
