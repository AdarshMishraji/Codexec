import CodeMirror, { EditorView } from "@uiw/react-codemirror";
import { useEffect, useRef, useState } from "react";
import { ApiError, adminListLanguages, createSubmission, getSubmission } from "../../lib/api";
import { extensionForLanguage } from "../../lib/codeExtensions";
import { PLAYGROUND_API_KEY_KEY } from "../../lib/storage";
import type { PublicLanguage, SubmissionResponse, SubmissionStatus } from "../../lib/types";

const TERMINAL_STATUSES = new Set<SubmissionStatus>([
  "completed",
  "compile_error",
  "runtime_error",
  "time_limit_exceeded",
  "memory_limit_exceeded",
  "internal_error",
]);

// Wrap long lines instead of growing the editor's own width - without
// this, typing (or a long stdout/stderr line) pushes the CodeMirror
// scroller wider than its box, which visually grows the whole panel.
const WRAP_ONLY = [EditorView.lineWrapping];

function statusClass(status: SubmissionStatus): string {
  if (status === "completed") return "on";
  if (status === "queued" || status === "processing") return "off";
  if (status === "time_limit_exceeded" || status === "memory_limit_exceeded") return "warn";
  return "danger";
}

interface PlaygroundTabProps {
  token: string;
  onSessionExpired: () => void;
}

export default function PlaygroundTab({ token, onSessionExpired }: PlaygroundTabProps) {
  const [languages, setLanguages] = useState<PublicLanguage[]>([]);
  const [language, setLanguage] = useState("");
  const [apiKey, setApiKey] = useState(() => localStorage.getItem(PLAYGROUND_API_KEY_KEY) ?? "");
  const [sourceCode, setSourceCode] = useState("");
  const [stdin, setStdin] = useState("");
  const [expected, setExpected] = useState("");
  const [cpuMs, setCpuMs] = useState("");
  const [cpuCores, setCpuCores] = useState("");
  const [memKb, setMemKb] = useState("");

  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SubmissionResponse | null>(null);
  const pollGeneration = useRef(0);

  useEffect(() => {
    adminListLanguages(token)
      .then((langs) => {
        const active = langs.filter((l) => l.is_active);
        setLanguages(active);
        setLanguage((prev) => (active.some((l) => l.slug === prev) ? prev : (active[0]?.slug ?? "")));
      })
      .catch((e) => {
        if (e instanceof ApiError && e.status === 401) onSessionExpired();
        // otherwise: left as-is, a background refresh failing isn't worth its own banner
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  function updateApiKey(v: string) {
    setApiKey(v);
    localStorage.setItem(PLAYGROUND_API_KEY_KEY, v);
  }

  async function pollSubmission(id: string, generation: number) {
    for (let attempt = 0; attempt < 90; attempt++) {
      if (pollGeneration.current !== generation) return; // a newer run superseded this one
      const sub = await getSubmission(apiKey, id);
      if (pollGeneration.current !== generation) return;
      setResult(sub);
      if (TERMINAL_STATUSES.has(sub.status)) return;
      await new Promise((r) => setTimeout(r, 1000));
    }
  }

  async function run() {
    setError(null);
    if (!language) {
      setError("No active plugin selected.");
      return;
    }
    if (!sourceCode.trim()) {
      setError("Source code can't be empty.");
      return;
    }
    if (!apiKey.trim()) {
      setError("An API key is required — create one in the API Keys tab and paste it here.");
      return;
    }

    const body: Parameters<typeof createSubmission>[1] = { language, source_code: sourceCode, stdin };
    if (expected.trim()) body.expected_output = expected;
    if (cpuMs) body.cpu_time_limit_ms = parseInt(cpuMs, 10);
    if (cpuCores) body.cpu_limit_cores = parseFloat(cpuCores);
    if (memKb) body.memory_limit_kb = parseInt(memKb, 10);

    const generation = ++pollGeneration.current;
    setRunning(true);
    setResult(null);
    try {
      const submitted = await createSubmission(apiKey, body);
      setResult({
        id: submitted.id,
        language,
        status: "queued",
        verdict: null,
        stdout: "",
        stderr: "",
        compile_output: "",
        exit_code: null,
        cpu_time_limit_ms: 0,
        cpu_limit_cores: 0,
        memory_limit_kb: 0,
        cpu_time_used_ms: null,
        memory_used_kb: null,
        limit_exceeded: null,
        error_message: null,
        submitted_at: submitted.submitted_at,
        started_at: null,
        finished_at: null,
      });
      await pollSubmission(submitted.id, generation);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
  }

  const meta = (() => {
    if (!result) return "";
    const parts: string[] = [];
    if (result.exit_code !== null) parts.push(`exit code ${result.exit_code}`);
    if (result.cpu_time_used_ms !== null) parts.push(`${result.cpu_time_used_ms.toFixed(1)} ms CPU`);
    if (result.memory_used_kb !== null) parts.push(`${(result.memory_used_kb / 1024).toFixed(1)} MB peak`);
    if (result.limit_exceeded) parts.push(`limit exceeded: ${result.limit_exceeded}`);
    return parts.length ? parts.join(" · ") : "Waiting for a worker to pick this up…";
  })();

  return (
    <section>
      <div className="playground-grid">
        <div className="panel">
          <div className="toolbar">
            <h2>Run a submission</h2>
          </div>
          {error && <div className="banner error">{error}</div>}
          <div className="form-row">
            <div>
              <label className="field">Language</label>
              <select value={language} onChange={(e) => setLanguage(e.target.value)}>
                {languages.length === 0 && <option value="">No active plugins — register one first</option>}
                {languages.map((l) => (
                  <option key={l.slug} value={l.slug}>
                    {l.display_name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="field">API key</label>
              <input
                type="password"
                placeholder="paste a raw key from the API Keys tab"
                autoComplete="off"
                value={apiKey}
                onChange={(e) => updateApiKey(e.target.value)}
              />
            </div>
          </div>
          <div className="form-row single">
            <div>
              <label className="field">Source code</label>
              <div className="editor-box">
                <CodeMirror
                  value={sourceCode}
                  height="260px"
                  theme="dark"
                  extensions={[...extensionForLanguage(language), EditorView.lineWrapping]}
                  onChange={setSourceCode}
                />
              </div>
            </div>
          </div>
          <div className="form-row">
            <div>
              <label className="field">Stdin</label>
              <textarea rows={3} placeholder="(optional)" value={stdin} onChange={(e) => setStdin(e.target.value)} />
            </div>
            <div>
              <label className="field">Expected output</label>
              <textarea
                rows={3}
                placeholder="(optional — enables accepted / wrong_answer grading)"
                value={expected}
                onChange={(e) => setExpected(e.target.value)}
              />
            </div>
          </div>
          <div className="form-row triple">
            <div>
              <label className="field">CPU time (ms)</label>
              <input type="number" placeholder="default" value={cpuMs} onChange={(e) => setCpuMs(e.target.value)} />
            </div>
            <div>
              <label className="field">CPU cores</label>
              <input
                type="number"
                step="0.1"
                placeholder="default"
                value={cpuCores}
                onChange={(e) => setCpuCores(e.target.value)}
              />
            </div>
            <div>
              <label className="field">Memory (KB)</label>
              <input type="number" placeholder="default" value={memKb} onChange={(e) => setMemKb(e.target.value)} />
            </div>
          </div>
          <button className="primary" style={{ width: "100%", marginTop: 8 }} disabled={running} onClick={run}>
            {running ? "Running…" : "Run"}
          </button>
        </div>

        {result && (
          <div className="panel">
            <div className="toolbar">
              <h2>Result</h2>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <span className={"badge " + statusClass(result.status)}>{result.status}</span>
                {result.verdict && (
                  <span className={"badge " + (result.verdict === "accepted" ? "on" : "danger")}>
                    {result.verdict}
                  </span>
                )}
              </div>
            </div>
            <div className="pg-meta">{meta}</div>
            {result.error_message && <div className="banner error">{result.error_message}</div>}
            <div className="output-block">
              <label className="field">Compile output</label>
              <div className="editor-box output">
                <CodeMirror value={result.compile_output ?? ""} height="110px" theme="dark" readOnly basicSetup={{ lineNumbers: false }} extensions={WRAP_ONLY} />
              </div>
            </div>
            <div className="output-block">
              <label className="field">Stdout</label>
              <div className="editor-box output">
                <CodeMirror value={result.stdout ?? ""} height="110px" theme="dark" readOnly basicSetup={{ lineNumbers: false }} extensions={WRAP_ONLY} />
              </div>
            </div>
            <div className="output-block">
              <label className="field">Stderr</label>
              <div className="editor-box output">
                <CodeMirror value={result.stderr ?? ""} height="110px" theme="dark" readOnly basicSetup={{ lineNumbers: false }} extensions={WRAP_ONLY} />
              </div>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
