import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";

// Ported 1:1 from crates/codexec-api/assets/docs.html (static HTML/CSS/JS page).
// Structure, ids, hrefs and copy are preserved verbatim; only the runtime
// behaviors (origin substitution, copy buttons, scrollspy) were rewritten as
// React idioms. Styling lives in ../theme.css under "Docs page" - it already
// mirrors the original inline <style> block, so this file only uses classNames.

function MethodBadge({ method }: { method: "GET" | "POST" | "DELETE" }) {
  return (
    <span className={`method-badge ${method.toLowerCase()}`}>{method}</span>
  );
}

function AuthBadge({ kind }: { kind: "key" | "admin" | "public" }) {
  const label =
    kind === "key"
      ? "Requires API key"
      : kind === "admin"
        ? "Requires admin token"
        : "Public";
  return <span className={`badge auth-${kind}`}>{label}</span>;
}

function StatusRow({ code, children }: { code: number; children: ReactNode }) {
  return (
    <div className="status-row">
      <span className={`status-pill s${Math.floor(code / 100)}`}>{code}</span>
      <span className="status-desc">{children}</span>
    </div>
  );
}

/** One code example with a "Copy" button - the react-idiomatic replacement for
 * the original's `.code-block-wrap` + `.copy-code-btn` + clipboard script. */
function CodeBlock({ children }: { children: string }) {
  const [copied, setCopied] = useState(false);
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    };
  }, []);

  const handleCopy = () => {
    navigator.clipboard?.writeText(children).then(() => {
      setCopied(true);
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
      timeoutRef.current = window.setTimeout(() => setCopied(false), 1400);
    });
  };

  return (
    <div className="code-block-wrap">
      <button className="copy-code-btn" type="button" onClick={handleCopy}>
        {copied ? "Copied!" : "Copy"}
      </button>
      <pre>
        <code>{children}</code>
      </pre>
    </div>
  );
}

export default function Docs() {
  const origin = window.location.origin;
  const sidebarRef = useRef<HTMLElement>(null);

  // Sidebar scrollspy: ported from the original's IntersectionObserver script.
  // Set up once after mount, direct DOM class toggling (no React state) since
  // it only ever touches the sidebar's own anchor elements.
  useEffect(() => {
    const nav = sidebarRef.current;
    if (!nav) return;

    const sideLinks = Array.from(
      nav.querySelectorAll<HTMLAnchorElement>(".side-link"),
    );
    const linkById = new Map(
      sideLinks.map((a) => [a.getAttribute("href")!.slice(1), a]),
    );
    const targets = sideLinks
      .map((a) => document.getElementById(a.getAttribute("href")!.slice(1)))
      .filter((el): el is HTMLElement => el !== null);

    if (!("IntersectionObserver" in window) || targets.length === 0) return;

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const link = linkById.get(entry.target.id);
          if (!link || !entry.isIntersecting) return;
          sideLinks.forEach((l) => l.classList.remove("active"));
          link.classList.add("active");
        });
      },
      { rootMargin: "-10% 0px -75% 0px", threshold: 0 },
    );

    targets.forEach((t) => observer.observe(t));
    return () => observer.disconnect();
  }, []);

  return (
    <div className="docs-shell">
      <nav className="sidebar" id="sidebar" ref={sidebarRef}>
        <a className="side-brand" href="/">
          <img className="brand-mark" src="/icon.png" alt="" />
          <span className="name">Codexec</span>
        </a>
        <div className="side-links">
          <a href="/">Dashboard</a>
          <a href="/admin">Admin Portal</a>
        </div>

        <div className="side-group">
          <div className="side-group-label">Overview</div>
          <a href="#authentication" className="side-link">
            Authentication
          </a>
          <a href="#errors" className="side-link">
            Errors
          </a>
        </div>
        <div className="side-group">
          <div className="side-group-label">Submissions</div>
          <a href="#submit-code" className="side-link">
            Submit code
          </a>
          <a href="#get-submission" className="side-link">
            Get a submission
          </a>
        </div>
        <div className="side-group">
          <div className="side-group-label">Languages</div>
          <a href="#list-languages" className="side-link">
            List languages
          </a>
        </div>
        <div className="side-group">
          <div className="side-group-label">Platform stats</div>
          <a href="#get-stats" className="side-link">
            Get stats
          </a>
        </div>
        <div className="side-group">
          <div className="side-group-label">Admin · Plugins</div>
          <a href="#admin-list-languages" className="side-link">
            List plugins
          </a>
          <a href="#admin-register-language" className="side-link">
            Register / update
          </a>
          <a href="#admin-activate-language" className="side-link">
            Activate
          </a>
          <a href="#admin-deactivate-language" className="side-link">
            Deactivate
          </a>
          <a href="#admin-delete-language" className="side-link">
            Delete
          </a>
        </div>
        <div className="side-group">
          <div className="side-group-label">Admin · Templates</div>
          <a href="#admin-list-templates" className="side-link">
            List templates
          </a>
          <a href="#admin-get-template" className="side-link">
            Get a template
          </a>
        </div>
        <div className="side-group">
          <div className="side-group-label">Admin · API Keys</div>
          <a href="#admin-list-keys" className="side-link">
            List keys
          </a>
          <a href="#admin-create-key" className="side-link">
            Create a key
          </a>
          <a href="#admin-delete-key" className="side-link">
            Delete a key
          </a>
        </div>
      </nav>

      <main>
        <div className="docs-header">
          <h1>codexec API</h1>
          <p className="lead">
            A REST API for submitting source code to an isolated,
            resource-limited sandbox (CPU time, CPU rate, and memory are all
            independently enforced) and retrieving the result. Every request and
            response body is JSON.
          </p>
          <div className="base-url-row">
            <span className="label">Base URL</span>
            <code id="base-url">{origin}</code>
          </div>
          <div className="quick-links">
            <a href="/">← Live dashboard</a>
            <a href="/admin">Admin Portal (manage plugins & keys)</a>
            <a href="/admin">Playground (try requests live) →</a>
          </div>
        </div>

        <section className="doc-section" id="authentication">
          <h2>Authentication</h2>
          <p className="section-desc">
            Every non-public endpoint uses the same scheme — an{" "}
            <code>{"Authorization: Bearer <token>"}</code> header — but there
            are two independent tokens, and they are not interchangeable.
          </p>

          <table className="fields" style={{ marginBottom: 18 }}>
            <thead>
              <tr>
                <th>Scheme</th>
                <th>Header</th>
                <th>Protects</th>
                <th>How to obtain it</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="f-name">API key</td>
                <td className="f-type">Authorization: Bearer cxk_…</td>
                <td>
                  All <code>/submissions</code> routes
                </td>
                <td>
                  Admin Portal → API Keys tab → Create Key (or{" "}
                  <code>POST /admin/api-keys</code> below). Shown once at
                  creation — only its hash is stored server-side, so a lost key
                  can't be recovered, only revoked and replaced.
                </td>
              </tr>
              <tr>
                <td className="f-name">Admin token</td>
                <td className="f-type">{"Authorization: Bearer <token>"}</td>
                <td>
                  All <code>/admin/*</code> routes
                </td>
                <td>
                  A single static value set by whoever deploys codexec, via the{" "}
                  <code>ADMIN_API_TOKEN</code> environment variable. Not created
                  or rotated through the API.
                </td>
              </tr>
            </tbody>
          </table>

          <div className="callout">
            <strong>Public, no auth required:</strong> <code>GET /</code>,{" "}
            <code>GET /admin</code>, <code>GET /docs</code>,{" "}
            <code>GET /stats</code>, and <code>GET /languages</code>. Everything
            else requires one of the two tokens above.
          </div>
          <div className="callout warn">
            <strong>Body requests need a real Content-Type.</strong> Every{" "}
            <code>POST</code> below expects{" "}
            <code>Content-Type: application/json</code>. Omit it and you get a
            framework-level <code>415</code> with a plain-text body (
            <code>Expected request with `Content-Type: application/json`</code>)
            — not the JSON error envelope described below. Malformed JSON
            similarly returns a plain-text <code>400</code> (e.g.{" "}
            <code>
              Failed to parse the request body as JSON: key must be a string at
              line 1 column 2
            </code>
            ). Both are framework-level responses, verified against the live
            server, not the <code>{'{"error", "message"}'}</code> shape used
            everywhere else in this document.
          </div>
        </section>

        <section className="doc-section" id="errors">
          <h2>Errors</h2>
          <p className="section-desc">
            Outside the Content-Type/malformed-JSON edge case above, every error
            response (any status ≥ 400) uses the same envelope:
          </p>
          <CodeBlock>{`{
  "error": "unknown_language",
  "message": "No active plugin registered for language 'cobol'"
}`}</CodeBlock>
          <p className="section-desc" style={{ marginTop: -6 }}>
            <code>error</code> is a stable machine-readable code — safe to
            switch on in client code. <code>message</code> is a human-readable
            detail that may change wording over time.
          </p>

          <table className="fields">
            <thead>
              <tr>
                <th>error</th>
                <th>Status</th>
                <th>Meaning</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="f-name">unknown_language</td>
                <td className="f-type">422</td>
                <td>
                  The <code>language</code> slug doesn't exist, or exists but is
                  deactivated. Both cases look identical to the caller by
                  design.
                </td>
              </tr>
              <tr>
                <td className="f-name">invalid_cpu_time_limit</td>
                <td className="f-type">422</td>
                <td>
                  <code>cpu_time_limit_ms</code> is ≤ 0 or exceeds the effective
                  ceiling for this language.
                </td>
              </tr>
              <tr>
                <td className="f-name">invalid_cpu_limit_cores</td>
                <td className="f-type">422</td>
                <td>
                  <code>cpu_limit_cores</code> is ≤ 0 or exceeds the effective
                  ceiling for this language.
                </td>
              </tr>
              <tr>
                <td className="f-name">invalid_memory_limit</td>
                <td className="f-type">422</td>
                <td>
                  <code>memory_limit_kb</code> is ≤ 0 or exceeds the effective
                  ceiling for this language.
                </td>
              </tr>
              <tr>
                <td className="f-name">source_too_large</td>
                <td className="f-type">422</td>
                <td>
                  <code>source_code</code> is empty, or it (or{" "}
                  <code>expected_output</code>) exceeds{" "}
                  <code>MAX_SOURCE_CODE_BYTES</code> (default 65536 bytes).
                </td>
              </tr>
              <tr>
                <td className="f-name">invalid_label</td>
                <td className="f-type">422</td>
                <td>
                  An API key's <code>label</code> was empty or all whitespace.
                </td>
              </tr>
              <tr>
                <td className="f-name">invalid_api_key</td>
                <td className="f-type">401</td>
                <td>
                  Missing, unrecognized, or revoked API key on a `/submissions`
                  route.
                </td>
              </tr>
              <tr>
                <td className="f-name">unauthorized</td>
                <td className="f-type">401</td>
                <td>
                  Missing or incorrect admin token on an `/admin/*` route.
                </td>
              </tr>
              <tr>
                <td className="f-name">not_found</td>
                <td className="f-type">404</td>
                <td>No resource exists at the given id/slug.</td>
              </tr>
              <tr>
                <td className="f-name">conflict</td>
                <td className="f-type">409</td>
                <td>
                  The request is well-formed but can't be applied right now —
                  currently only returned when deleting a plugin that has
                  submission history.
                </td>
              </tr>
              <tr>
                <td className="f-name">queue_unavailable</td>
                <td className="f-type">503</td>
                <td>
                  The submission couldn't be published to the execution queue;
                  nothing was persisted — safe to retry.
                </td>
              </tr>
              <tr>
                <td className="f-name">internal_error</td>
                <td className="f-type">500</td>
                <td>
                  An unexpected server-side failure (e.g. a database error).
                </td>
              </tr>
            </tbody>
          </table>
        </section>

        <section className="doc-section" id="submissions">
          <h2>Submissions</h2>
          <p className="section-desc">
            The core of the API: submit source code for execution, then poll for
            the result. Submitting is asynchronous — there is no synchronous
            "run and wait" mode — so a typical integration submits once and
            polls <a href="#get-submission">{"GET /submissions/{id}"}</a> every
            second or so until the status is terminal.
          </p>

          <div className="panel endpoint" id="submit-code">
            <div className="endpoint-head">
              <MethodBadge method="POST" />
              <code className="endpoint-path">/submissions</code>
            </div>
            <p className="endpoint-desc">
              Queues a new submission for execution and returns immediately.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="key" />
            </div>

            <h4 className="sub-head">Request body</h4>
            <table className="fields">
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Type</th>
                  <th>Required</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="f-name">language</td>
                  <td className="f-type">string</td>
                  <td>
                    <span className="req-yes">required</span>
                  </td>
                  <td>
                    An active language's <code>slug</code> (see{" "}
                    <a href="#list-languages">GET /languages</a>).
                  </td>
                </tr>
                <tr>
                  <td className="f-name">source_code</td>
                  <td className="f-type">string</td>
                  <td>
                    <span className="req-yes">required</span>
                  </td>
                  <td>
                    Non-empty, up to <code>MAX_SOURCE_CODE_BYTES</code> (default
                    65536 bytes).
                  </td>
                </tr>
                <tr>
                  <td className="f-name">stdin</td>
                  <td className="f-type">string</td>
                  <td>
                    <span className="req-no">optional</span>
                  </td>
                  <td>
                    Defaults to <code>""</code>. Piped to the program's standard
                    input.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">expected_output</td>
                  <td className="f-type">string | null</td>
                  <td>
                    <span className="req-no">optional</span>
                  </td>
                  <td>
                    When set, enables grading: <code>verdict</code> becomes{" "}
                    <code>"accepted"</code> or <code>"wrong_answer"</code> once
                    the run completes (exact match after trimming trailing
                    whitespace per line and trailing blank lines). Left{" "}
                    <code>null</code>, the submission is execution-only and{" "}
                    <code>verdict</code> stays <code>null</code> forever.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">cpu_time_limit_ms</td>
                  <td className="f-type">integer | null</td>
                  <td>
                    <span className="req-no">optional</span>
                  </td>
                  <td>
                    Defaults to the language's{" "}
                    <code>default_cpu_time_limit_ms</code>. Must be in{" "}
                    <code>(0, effective_max]</code>, where the effective max is{" "}
                    <code>
                      min(language.max_cpu_time_limit_ms, platform ceiling)
                    </code>
                    .
                  </td>
                </tr>
                <tr>
                  <td className="f-name">cpu_limit_cores</td>
                  <td className="f-type">number | null</td>
                  <td>
                    <span className="req-no">optional</span>
                  </td>
                  <td>
                    Defaults to the language's{" "}
                    <code>default_cpu_limit_cores</code>. Same ceiling rule as
                    above, against <code>max_cpu_limit_cores</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">memory_limit_kb</td>
                  <td className="f-type">integer | null</td>
                  <td>
                    <span className="req-no">optional</span>
                  </td>
                  <td>
                    Defaults to the language's{" "}
                    <code>default_memory_limit_kb</code>. Same ceiling rule as
                    above, against <code>max_memory_limit_kb</code>.
                  </td>
                </tr>
              </tbody>
            </table>
            <div className="callout">
              <strong>Two independent CPU controls.</strong>{" "}
              <code>cpu_limit_cores</code> is a hard rate cap (kernel-enforced
              via the cgroup's CFS quota — e.g. <code>1.5</code> cores throttles
              concurrent CPU draw). <code>cpu_time_limit_ms</code> is a total
              CPU-time budget the worker enforces by polling cumulative usage
              and killing the process once it's exceeded. A program can hit
              either independently of the other.
            </div>

            <h4 className="sub-head">Example request</h4>
            <CodeBlock>{`curl -X POST ${origin}/submissions \\
  -H "Authorization: Bearer cxk_your_api_key" \\
  -H "Content-Type: application/json" \\
  -d '{
    "language": "python3",
    "source_code": "print(\\"hello, world\\")",
    "stdin": "",
    "expected_output": null,
    "cpu_time_limit_ms": 2000,
    "cpu_limit_cores": 1.0,
    "memory_limit_kb": 262144
  }'`}</CodeBlock>

            <StatusRow code={202}>Accepted — queued</StatusRow>
            <CodeBlock>{`{
  "id": "3583fc02-4500-45e7-9eaf-1455fe7fc9fb",
  "status": "queued",
  "submitted_at": "2026-09-12T07:28:10.806118Z"
}`}</CodeBlock>

            <StatusRow code={401}>
              <code>invalid_api_key</code> — missing, unknown, or revoked key
            </StatusRow>
            <StatusRow code={422}>
              <code>unknown_language</code> /{" "}
              <code>invalid_cpu_time_limit</code> /{" "}
              <code>invalid_cpu_limit_cores</code> /{" "}
              <code>invalid_memory_limit</code> / <code>source_too_large</code>
            </StatusRow>
            <StatusRow code={503}>
              <code>queue_unavailable</code> — nothing was persisted; safe to
              retry
            </StatusRow>
          </div>

          <div className="panel endpoint" id="get-submission">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">{"/submissions/{id}"}</code>
            </div>
            <p className="endpoint-desc">
              Fetches the current state of a submission. Poll this until{" "}
              <code>status</code> is terminal.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="key" />
            </div>

            <h4 className="sub-head">Path parameters</h4>
            <table className="fields">
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Type</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="f-name">id</td>
                  <td className="f-type">UUID</td>
                  <td>
                    The id returned by{" "}
                    <a href="#submit-code">POST /submissions</a>.
                  </td>
                </tr>
              </tbody>
            </table>

            <h4 className="sub-head">Example request</h4>
            <CodeBlock>{`curl ${origin}/submissions/3583fc02-4500-45e7-9eaf-1455fe7fc9fb \\
  -H "Authorization: Bearer cxk_your_api_key"`}</CodeBlock>

            <StatusRow code={200}>OK</StatusRow>
            <CodeBlock>{`{
  "id": "c92a8bb8-f06f-4a36-9bd4-8c444e4d2725",
  "language": "python3",
  "status": "completed",
  "verdict": null,
  "stdout": "2\\n",
  "stderr": "",
  "compile_output": null,
  "exit_code": 0,
  "cpu_time_limit_ms": 2000,
  "cpu_limit_cores": 1.0,
  "memory_limit_kb": 262144,
  "cpu_time_used_ms": 22.0,
  "memory_used_kb": 4304,
  "limit_exceeded": null,
  "error_message": null,
  "submitted_at": "2026-09-12T07:28:10.806118Z",
  "started_at": "2026-09-12T07:28:10.815465Z",
  "finished_at": "2026-09-12T07:28:10.881975Z"
}`}</CodeBlock>

            <h4 className="sub-head">Response fields</h4>
            <table className="fields">
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Type</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="f-name">status</td>
                  <td className="f-type">enum</td>
                  <td>
                    One of <code>queued</code>, <code>processing</code>,{" "}
                    <code>completed</code>, <code>compile_error</code>,{" "}
                    <code>runtime_error</code>, <code>time_limit_exceeded</code>
                    , <code>memory_limit_exceeded</code>,{" "}
                    <code>internal_error</code>. Anything other than{" "}
                    <code>queued</code>/<code>processing</code> is terminal.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">verdict</td>
                  <td className="f-type">enum | null</td>
                  <td>
                    <code>accepted</code> or <code>wrong_answer</code> — set
                    only if the submission carried <code>expected_output</code>{" "}
                    <em>and</em> reached <code>completed</code>. Stays{" "}
                    <code>null</code> for every other status, even if{" "}
                    <code>expected_output</code> was provided.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">limit_exceeded</td>
                  <td className="f-type">enum | null</td>
                  <td>
                    <code>cpu_time</code> when{" "}
                    <code>status = time_limit_exceeded</code>,{" "}
                    <code>memory</code> when{" "}
                    <code>status = memory_limit_exceeded</code>, otherwise{" "}
                    <code>null</code>. Derived from <code>status</code>, not an
                    independent signal.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">stdout</td>
                  <td className="f-type">string | null</td>
                  <td>
                    Captured standard output. May be truncated at a
                    platform-defined byte ceiling.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">stderr</td>
                  <td className="f-type">string | null</td>
                  <td>Captured standard error.</td>
                </tr>
                <tr>
                  <td className="f-name">compile_output</td>
                  <td className="f-type">string | null</td>
                  <td>
                    Captured compiler output, for languages with a compile step.{" "}
                    <code>null</code> for interpreted languages or before
                    compilation runs.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">exit_code</td>
                  <td className="f-type">integer | null</td>
                  <td>The program's process exit code, once it has run.</td>
                </tr>
                <tr>
                  <td className="f-name">cpu_time_used_ms</td>
                  <td className="f-type">number | null</td>
                  <td>
                    Actual cumulative CPU time consumed, read from the cgroup —
                    not wall-clock time.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">memory_used_kb</td>
                  <td className="f-type">integer | null</td>
                  <td>Peak resident memory, read from the cgroup.</td>
                </tr>
                <tr>
                  <td className="f-name">error_message</td>
                  <td className="f-type">string | null</td>
                  <td>
                    Set only when <code>status = internal_error</code> — details
                    of the platform-side failure.
                  </td>
                </tr>
              </tbody>
            </table>

            <StatusRow code={401}>
              <code>invalid_api_key</code>
            </StatusRow>
            <StatusRow code={404}>
              <code>not_found</code> — no such submission
            </StatusRow>
          </div>
        </section>

        <section className="doc-section" id="languages">
          <h2>Languages</h2>
          <p className="section-desc">
            Public, read-only. Lists every currently-active plugin along with
            the limits your submissions to it are validated against.
          </p>

          <div className="panel endpoint" id="list-languages">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">/languages</code>
            </div>
            <p className="endpoint-desc">
              Active languages only — inactive/unregistered ones never appear
              here, and behave identically to an unknown slug from{" "}
              <a href="#submit-code">POST /submissions</a>' point of view.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="public" />
            </div>

            <h4 className="sub-head">Example request</h4>
            <CodeBlock>{`curl ${origin}/languages`}</CodeBlock>

            <StatusRow code={200}>
              OK — array, one entry per active language
            </StatusRow>
            <CodeBlock>{`[
  {
    "slug": "python3",
    "display_name": "Python 3.11",
    "version": "3.11.4",
    "default_cpu_time_limit_ms": 2000,
    "default_cpu_limit_cores": 1.0,
    "default_memory_limit_kb": 262144,
    "max_cpu_time_limit_ms": 10000,
    "max_cpu_limit_cores": 2.0,
    "max_memory_limit_kb": 1048576
  }
]`}</CodeBlock>
            <p className="section-desc">
              Note what's <em>not</em> here: <code>image_ref</code>,{" "}
              <code>compile_cmd</code>, and <code>run_cmd</code> are internal
              implementation details, only exposed via the authenticated{" "}
              <a href="#admin-list-languages">admin listing</a>.
            </p>
          </div>
        </section>

        <section className="doc-section" id="stats">
          <h2>Platform stats</h2>
          <p className="section-desc">
            Public, read-only aggregate statistics — the same endpoint the{" "}
            <a href="/">live dashboard</a> polls every 15 seconds. Every field
            is either an aggregate number or submission <em>metadata</em>;
            source code and stdout/stderr content are never included here (they
            require an API key via{" "}
            <a href="#get-submission">{"GET /submissions/{id}"}</a>).
          </p>

          <div className="panel endpoint" id="get-stats">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">/stats</code>
            </div>
            <div className="endpoint-meta">
              <AuthBadge kind="public" />
            </div>

            <h4 className="sub-head">Example request</h4>
            <CodeBlock>{`curl ${origin}/stats`}</CodeBlock>

            <StatusRow code={200}>OK</StatusRow>
            <CodeBlock>{`{
  "total_submissions": 1042,
  "completed_submissions": 918,
  "success_rate_pct": 88.1,
  "submissions_last_24h": 37,
  "submissions_last_7d": 210,
  "avg_cpu_time_ms": 143.6,
  "avg_memory_kb": 18342.2,
  "p50_cpu_time_ms": 41.0,
  "p95_cpu_time_ms": 612.0,
  "avg_wall_time_ms": 96.4,
  "avg_queue_wait_ms": 8.2,
  "languages_total": 12,
  "languages_active": 11,
  "api_keys_total": 4,
  "api_keys_active": 3,
  "status_breakdown": [ { "status": "completed", "count": 918 }, { "status": "runtime_error", "count": 64 } ],
  "verdict_breakdown": [ { "verdict": "accepted", "count": 310 }, { "verdict": "wrong_answer", "count": 52 } ],
  "language_breakdown": [ { "slug": "python3", "display_name": "Python 3.11", "count": 402 } ],
  "daily_trend": [ { "day": "2026-08-30", "count": 51 }, { "day": "2026-08-31", "count": 64 } ],
  "recent_submissions": [
    {
      "id": "c92a8bb8-f06f-4a36-9bd4-8c444e4d2725",
      "language_slug": "python3",
      "status": "completed",
      "verdict": null,
      "cpu_time_used_ms": 22.0,
      "memory_used_kb": 4304,
      "submitted_at": "2026-09-12T07:28:10.806118Z",
      "finished_at": "2026-09-12T07:28:10.881975Z"
    }
  ]
}`}</CodeBlock>
            <p className="section-desc">
              <code>daily_trend</code> covers the last 14 days, zero-filled for
              days with no submissions. <code>recent_submissions</code> is
              capped at the 20 most recent, metadata only.
            </p>
          </div>
        </section>

        <section className="doc-section" id="admin">
          <h2>Admin API</h2>
          <p className="section-desc">
            Everything under <code>/admin</code> requires the admin bearer token
            (see <a href="#authentication">Authentication</a>) and is meant for
            operators managing the platform — registering plugins and issuing
            API keys — rather than end-user integrations. The{" "}
            <a href="/admin">Admin Portal</a> is a full UI over this same API,
            including a Playground for live testing.
          </p>

          <h4 className="sub-head section-label" id="admin-list-languages">
            Plugins
          </h4>

          <div className="panel endpoint">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">/admin/languages</code>
            </div>
            <p className="endpoint-desc">
              Every registered language, active and inactive, with full detail
              (image reference, argv commands, timestamps).
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl ${origin}/admin/languages \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={200}>OK — array of full language rows</StatusRow>
            <CodeBlock>{`[
  {
    "id": "8f1a2e3d-...",
    "slug": "python3",
    "display_name": "Python 3.11",
    "version": "3.11.4",
    "image_ref": "docker.io/library/python:3.11-slim",
    "compile_cmd": null,
    "run_cmd": ["python3", "main.py"],
    "source_filename": "main.py",
    "compile_time_limit_ms": 10000,
    "default_cpu_time_limit_ms": 2000,
    "default_cpu_limit_cores": 1.0,
    "default_memory_limit_kb": 262144,
    "max_cpu_time_limit_ms": 10000,
    "max_cpu_limit_cores": 2.0,
    "max_memory_limit_kb": 1048576,
    "is_active": true,
    "created_at": "2026-09-02T19:22:00Z",
    "updated_at": "2026-09-02T19:22:00Z"
  }
]`}</CodeBlock>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
          </div>

          <div className="panel endpoint" id="admin-register-language">
            <div className="endpoint-head">
              <MethodBadge method="POST" />
              <code className="endpoint-path">/admin/languages</code>
            </div>
            <p className="endpoint-desc">
              Registers a new plugin, or upserts an existing one by{" "}
              <code>slug</code> — re-registering an existing slug overwrites
              every field and forces it back to active. Only writes the database
              row; a worker pulls the image itself in the background the first
              time it sees a slug it doesn't have cached.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>

            <h4 className="sub-head">Request body</h4>
            <table className="fields">
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Type</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="f-name">language.slug</td>
                  <td className="f-type">string</td>
                  <td>
                    Unique identifier, e.g. <code>"python3"</code>. Used in{" "}
                    <a href="#submit-code">POST /submissions</a>'{" "}
                    <code>language</code> field.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">language.display_name</td>
                  <td className="f-type">string</td>
                  <td>Human-readable name shown in the dashboard/admin UI.</td>
                </tr>
                <tr>
                  <td className="f-name">language.version</td>
                  <td className="f-type">string</td>
                  <td>Free-form version string.</td>
                </tr>
                <tr>
                  <td className="f-name">image.reference</td>
                  <td className="f-type">string</td>
                  <td>
                    A pullable OCI image reference (e.g. a public Docker Hub
                    tag).
                  </td>
                </tr>
                <tr>
                  <td className="f-name">commands.compile_cmd</td>
                  <td className="f-type">string[]</td>
                  <td>
                    Argv array, run before <code>run_cmd</code>. Empty array for
                    interpreted languages with no compile step.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">commands.run_cmd</td>
                  <td className="f-type">string[]</td>
                  <td>Argv array that executes the submission.</td>
                </tr>
                <tr>
                  <td className="f-name">commands.source_filename</td>
                  <td className="f-type">string</td>
                  <td>
                    Filename the submitted <code>source_code</code> is written
                    to before <code>compile_cmd</code>/<code>run_cmd</code> run,
                    e.g. <code>"main.py"</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">commands.compile_time_limit_ms</td>
                  <td className="f-type">integer</td>
                  <td>Wall-clock ceiling for the compile step.</td>
                </tr>
                <tr>
                  <td className="f-name">limits.default_cpu_time_limit_ms</td>
                  <td className="f-type">integer</td>
                  <td>
                    Used when a submission omits <code>cpu_time_limit_ms</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">limits.default_cpu_limit_cores</td>
                  <td className="f-type">number</td>
                  <td>
                    Used when a submission omits <code>cpu_limit_cores</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">limits.default_memory_limit_kb</td>
                  <td className="f-type">integer</td>
                  <td>
                    Used when a submission omits <code>memory_limit_kb</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">limits.max_cpu_time_limit_ms</td>
                  <td className="f-type">integer</td>
                  <td>
                    Per-language ceiling — combined with the platform-wide
                    ceiling via <code>min()</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">limits.max_cpu_limit_cores</td>
                  <td className="f-type">number</td>
                  <td>
                    Per-language ceiling — combined with the platform-wide
                    ceiling via <code>min()</code>.
                  </td>
                </tr>
                <tr>
                  <td className="f-name">limits.max_memory_limit_kb</td>
                  <td className="f-type">integer</td>
                  <td>
                    Per-language ceiling — combined with the platform-wide
                    ceiling via <code>min()</code>.
                  </td>
                </tr>
              </tbody>
            </table>

            <h4 className="sub-head">Example request</h4>
            <CodeBlock>{`curl -X POST ${origin}/admin/languages \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \\
  -H "Content-Type: application/json" \\
  -d '{
    "language": { "slug": "python3", "display_name": "Python 3.11", "version": "3.11.4" },
    "image": { "reference": "docker.io/library/python:3.11-slim" },
    "commands": {
      "compile_cmd": [],
      "run_cmd": ["python3", "main.py"],
      "source_filename": "main.py",
      "compile_time_limit_ms": 10000
    },
    "limits": {
      "default_cpu_time_limit_ms": 2000, "default_cpu_limit_cores": 1.0, "default_memory_limit_kb": 262144,
      "max_cpu_time_limit_ms": 10000, "max_cpu_limit_cores": 2.0, "max_memory_limit_kb": 1048576
    }
  }'`}</CodeBlock>
            <StatusRow code={200}>
              OK — the full language row (same shape as{" "}
              <a href="#admin-list-languages">GET /admin/languages</a>'s array
              entries)
            </StatusRow>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <div className="callout">
              Pre-built manifests for common languages (with real, tested{" "}
              <code>compile_cmd</code>/<code>run_cmd</code> values) are
              available via{" "}
              <a href="#admin-list-templates">the plugin templates endpoints</a>{" "}
              and the Admin Portal's "Add Plugin" form.
            </div>
          </div>

          <div className="panel endpoint" id="admin-activate-language">
            <div className="endpoint-head">
              <MethodBadge method="POST" />
              <code className="endpoint-path">
                {"/admin/languages/{slug}/activate"}
              </code>
            </div>
            <p className="endpoint-desc">
              Marks a plugin active, making it selectable in new submissions
              again.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl -X POST ${origin}/admin/languages/python3/activate \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={200}>OK — the updated language row</StatusRow>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <StatusRow code={404}>
              <code>not_found</code> — no such slug
            </StatusRow>
          </div>

          <div className="panel endpoint" id="admin-deactivate-language">
            <div className="endpoint-head">
              <MethodBadge method="POST" />
              <code className="endpoint-path">
                {"/admin/languages/{slug}/deactivate"}
              </code>
            </div>
            <p className="endpoint-desc">
              Marks a plugin inactive. New submissions to it are rejected with{" "}
              <code>unknown_language</code>; already-processing submissions
              finish normally, and history is untouched.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl -X POST ${origin}/admin/languages/python3/deactivate \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={200}>OK — the updated language row</StatusRow>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <StatusRow code={404}>
              <code>not_found</code> — no such slug
            </StatusRow>
          </div>

          <div className="panel endpoint" id="admin-delete-language">
            <div className="endpoint-head">
              <MethodBadge method="DELETE" />
              <code className="endpoint-path">{"/admin/languages/{slug}"}</code>
            </div>
            <p className="endpoint-desc">
              Permanently removes a plugin definition — distinct from
              deactivating. Only succeeds if no submission has ever referenced
              it.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl -X DELETE ${origin}/admin/languages/python3 \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={204}>No Content — deleted</StatusRow>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <StatusRow code={404}>
              <code>not_found</code> — no such slug
            </StatusRow>
            <StatusRow code={409}>
              <code>conflict</code> — has submission history; deactivate instead
            </StatusRow>
            <CodeBlock>{`{
  "error": "conflict",
  "message": "this plugin has existing submissions and can't be deleted - deactivate it instead"
}`}</CodeBlock>
          </div>

          <h4 className="sub-head section-label" id="admin-list-templates">
            Plugin templates
          </h4>
          <p className="section-desc">
            Read-only manifests for the plugins this deployment ships
            Dockerfiles for, whose images are published on Docker Hub — the same
            data source behind the Admin Portal's "start from an existing
            plugin" picker. Embedded at build time, not stored in the database.
          </p>

          <div className="panel endpoint">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">/admin/plugin-templates</code>
            </div>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl ${origin}/admin/plugin-templates \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={200}>OK</StatusRow>
            <CodeBlock>{`[
  { "slug": "c", "display_name": "C (GCC 14, gnu11)", "version": "14.2.0" },
  { "slug": "python3", "display_name": "Python 3.11", "version": "3.11.4" }
]`}</CodeBlock>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
          </div>

          <div className="panel endpoint" id="admin-get-template">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">
                {"/admin/plugin-templates/{slug}"}
              </code>
            </div>
            <p className="endpoint-desc">
              Returns the full manifest — same shape{" "}
              <a href="#admin-register-language">POST /admin/languages</a>{" "}
              expects as its body, so it can be submitted as-is or edited first.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl ${origin}/admin/plugin-templates/python3 \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={200}>
              OK — a <code>PluginManifest</code> (same shape as the register
              request body above)
            </StatusRow>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <StatusRow code={404}>
              <code>not_found</code> — no built-in template for that slug
            </StatusRow>
          </div>

          <h4 className="sub-head section-label" id="admin-list-keys">
            API keys
          </h4>

          <div className="panel endpoint">
            <div className="endpoint-head">
              <MethodBadge method="GET" />
              <code className="endpoint-path">/admin/api-keys</code>
            </div>
            <p className="endpoint-desc">
              Lists every API key. Never includes the raw key or its hash — only{" "}
              <code>key_prefix</code>, for identification.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl ${origin}/admin/api-keys \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={200}>OK</StatusRow>
            <CodeBlock>{`[
  {
    "id": "6da3494f-47f0-455a-81a1-bc7274628839",
    "label": "acme-corp-integration",
    "key_prefix": "cxk_e0d7bd3e",
    "is_active": true,
    "created_at": "2026-09-12T07:28:02.084943Z",
    "last_used_at": "2026-09-12T07:28:10.806118Z",
    "revoked_at": null
  }
]`}</CodeBlock>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
          </div>

          <div className="panel endpoint" id="admin-create-key">
            <div className="endpoint-head">
              <MethodBadge method="POST" />
              <code className="endpoint-path">/admin/api-keys</code>
            </div>
            <p className="endpoint-desc">
              Creates a new API key. The raw key is returned{" "}
              <strong>exactly once</strong>, in this response only — only its
              SHA-256 hash is stored, so it cannot be displayed again later.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>

            <h4 className="sub-head">Request body</h4>
            <table className="fields">
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Type</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="f-name">label</td>
                  <td className="f-type">string</td>
                  <td>
                    Non-empty, human-readable name to identify this key later
                    (e.g. <code>"acme-corp-integration"</code>).
                  </td>
                </tr>
              </tbody>
            </table>

            <h4 className="sub-head">Example request</h4>
            <CodeBlock>{`curl -X POST ${origin}/admin/api-keys \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \\
  -H "Content-Type: application/json" \\
  -d '{ "label": "acme-corp-integration" }'`}</CodeBlock>
            <StatusRow code={201}>Created</StatusRow>
            <CodeBlock>{`{
  "id": "6da3494f-47f0-455a-81a1-bc7274628839",
  "label": "acme-corp-integration",
  "key_prefix": "cxk_e0d7bd3e",
  "is_active": true,
  "created_at": "2026-09-12T07:28:02.084943Z",
  "last_used_at": null,
  "revoked_at": null,
  "api_key": "cxk_e0d7bd3eb08d4eb7b7e16f9ff15f0859"
}`}</CodeBlock>
            <p className="section-desc" style={{ marginTop: -8 }}>
              Store <code>api_key</code> (the full string) somewhere safe now —
              use it as the <code>Authorization: Bearer</code> value against{" "}
              <a href="#submit-code">/submissions</a>. Every other field here
              also appears in <a href="#admin-list-keys">GET /admin/api-keys</a>
              , but without <code>api_key</code>.
            </p>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <StatusRow code={422}>
              <code>invalid_label</code> — empty or whitespace-only label
            </StatusRow>
          </div>

          <div className="panel endpoint" id="admin-delete-key">
            <div className="endpoint-head">
              <MethodBadge method="DELETE" />
              <code className="endpoint-path">{"/admin/api-keys/{id}"}</code>
            </div>
            <p className="endpoint-desc">
              Permanently revokes a key. Any client using it loses access on its
              very next request — there is no grace period. Submissions already
              made with it keep their <code>api_key_id</code> attribution for
              history purposes.
            </p>
            <div className="endpoint-meta">
              <AuthBadge kind="admin" />
            </div>
            <CodeBlock>{`curl -X DELETE ${origin}/admin/api-keys/6da3494f-47f0-455a-81a1-bc7274628839 \\
  -H "Authorization: Bearer $ADMIN_API_TOKEN"`}</CodeBlock>
            <StatusRow code={204}>No Content — revoked</StatusRow>
            <StatusRow code={401}>
              <code>unauthorized</code>
            </StatusRow>
            <StatusRow code={404}>
              <code>not_found</code> — no such key id
            </StatusRow>
          </div>
        </section>

        <footer className="docs-footer">
          codexec API documentation — generated from the live route table. No
          pagination or rate limiting is currently enforced on any endpoint.
        </footer>
      </main>
    </div>
  );
}
