import { useState } from "react";
import ApiKeysTab from "./ApiKeysTab";
import PlaygroundTab from "./PlaygroundTab";
import PluginsTab from "./PluginsTab";

type Tab = "plugins" | "keys" | "playground";

interface AdminLayoutProps {
  token: string;
  onLock: () => void;
  onSessionExpired: () => void;
}

export default function AdminLayout({ token, onLock, onSessionExpired }: AdminLayoutProps) {
  const [tab, setTab] = useState<Tab>("plugins");

  return (
    <div className="wrap">
      <header className="top">
        <div className="brand">
          <img className="brand-mark" src="/icon.png" alt="" />
          <div>
            <h1>Codexec Admin</h1>
            <span className="sub">Plugins &amp; API key management</span>
          </div>
        </div>
        <div className="top-actions">
          <a className="link" href="/">
            ← Dashboard
          </a>
          <a className="link" href="/docs">
            API Docs
          </a>
          <button className="link" onClick={onLock}>
            Lock
          </button>
        </div>
      </header>

      <div className="tabs">
        <div className={"tab" + (tab === "plugins" ? " active" : "")} onClick={() => setTab("plugins")}>
          Plugins
        </div>
        <div className={"tab" + (tab === "keys" ? " active" : "")} onClick={() => setTab("keys")}>
          API Keys
        </div>
        <div className={"tab" + (tab === "playground" ? " active" : "")} onClick={() => setTab("playground")}>
          Playground
        </div>
      </div>

      {/* All three tabs stay mounted (just hidden) so in-progress form state -
          notably the Playground's typed code and settings - survives
          switching tabs, matching the original page's show/hide behavior. */}
      <div hidden={tab !== "plugins"}>
        <PluginsTab token={token} onSessionExpired={onSessionExpired} />
      </div>
      <div hidden={tab !== "keys"}>
        <ApiKeysTab token={token} onSessionExpired={onSessionExpired} />
      </div>
      <div hidden={tab !== "playground"}>
        <PlaygroundTab token={token} onSessionExpired={onSessionExpired} />
      </div>
    </div>
  );
}
