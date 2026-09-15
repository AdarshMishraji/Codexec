import { useEffect, useState } from "react";
import ChipInput from "../../components/ChipInput";
import Modal from "../../components/Modal";
import {
  ApiError,
  adminActivateLanguage,
  adminDeactivateLanguage,
  adminDeleteLanguage,
  adminGetPluginTemplate,
  adminListLanguages,
  adminListPluginTemplates,
  adminRegisterLanguage,
} from "../../lib/api";
import type { AdminLanguage, PluginManifest, PluginTemplateSummary } from "../../lib/types";

const EMPTY_FORM = {
  slug: "",
  displayName: "",
  version: "",
  image: "",
  compileCmd: [] as string[],
  runCmd: [] as string[],
  sourceFilename: "",
  compileTimeoutMs: "10000",
  defCpuMs: "2000",
  defCpuCores: "1.0",
  defMemKb: "262144",
  maxCpuMs: "10000",
  maxCpuCores: "2.0",
  maxMemKb: "1048576",
};

function manifestToForm(m: PluginManifest) {
  return {
    slug: m.language.slug,
    displayName: m.language.display_name,
    version: m.language.version,
    image: m.image.reference,
    compileCmd: m.commands.compile_cmd ?? [],
    runCmd: m.commands.run_cmd ?? [],
    sourceFilename: m.commands.source_filename,
    compileTimeoutMs: String(m.commands.compile_time_limit_ms),
    defCpuMs: String(m.limits.default_cpu_time_limit_ms),
    defCpuCores: String(m.limits.default_cpu_limit_cores),
    defMemKb: String(m.limits.default_memory_limit_kb),
    maxCpuMs: String(m.limits.max_cpu_time_limit_ms),
    maxCpuCores: String(m.limits.max_cpu_limit_cores),
    maxMemKb: String(m.limits.max_memory_limit_kb),
  };
}

interface PluginsTabProps {
  token: string;
  onSessionExpired: () => void;
}

export default function PluginsTab({ token, onSessionExpired }: PluginsTabProps) {
  const [languages, setLanguages] = useState<AdminLanguage[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [busySlug, setBusySlug] = useState<string | null>(null);

  const [modalOpen, setModalOpen] = useState(false);
  const [form, setForm] = useState(EMPTY_FORM);
  const [templates, setTemplates] = useState<PluginTemplateSummary[]>([]);
  const [selectedTemplate, setSelectedTemplate] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function load() {
    try {
      const langs = await adminListLanguages(token);
      setLanguages(langs);
      setListError(null);
    } catch (e) {
      if (onSessionExpired && e instanceof ApiError && e.status === 401) return onSessionExpired();
      setListError(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  async function toggleActive(lang: AdminLanguage) {
    setBusySlug(lang.slug);
    try {
      if (lang.is_active) await adminDeactivateLanguage(token, lang.slug);
      else await adminActivateLanguage(token, lang.slug);
      await load();
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return onSessionExpired();
      alert(e instanceof Error ? e.message : String(e));
    } finally {
      setBusySlug(null);
    }
  }

  async function removePlugin(lang: AdminLanguage) {
    if (
      !confirm(
        `Delete plugin "${lang.display_name}"? This only succeeds if it has no submission history — otherwise deactivate it instead.`,
      )
    )
      return;
    setBusySlug(lang.slug);
    try {
      await adminDeleteLanguage(token, lang.slug);
      await load();
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return onSessionExpired();
      alert(e instanceof Error ? e.message : String(e));
      setBusySlug(null);
    }
  }

  function openAddModal() {
    setForm(EMPTY_FORM);
    setSelectedTemplate("");
    setFormError(null);
    setModalOpen(true);
    adminListPluginTemplates(token)
      .then(setTemplates)
      .catch(() => setTemplates([])); // template picker just stays empty - not fatal
  }

  async function onTemplateChange(slug: string) {
    setSelectedTemplate(slug);
    if (!slug) return;
    setFormError(null);
    try {
      const manifest = await adminGetPluginTemplate(token, slug);
      setForm(manifestToForm(manifest));
    } catch (e) {
      setFormError(e instanceof Error ? e.message : String(e));
    }
  }

  async function submitPlugin() {
    setFormError(null);
    const manifest: PluginManifest = {
      language: {
        slug: form.slug.trim(),
        display_name: form.displayName.trim(),
        version: form.version.trim(),
      },
      image: { reference: form.image.trim() },
      commands: {
        compile_cmd: form.compileCmd,
        run_cmd: form.runCmd,
        source_filename: form.sourceFilename.trim(),
        compile_time_limit_ms: parseInt(form.compileTimeoutMs, 10),
      },
      limits: {
        default_cpu_time_limit_ms: parseInt(form.defCpuMs, 10),
        default_cpu_limit_cores: parseFloat(form.defCpuCores),
        default_memory_limit_kb: parseInt(form.defMemKb, 10),
        max_cpu_time_limit_ms: parseInt(form.maxCpuMs, 10),
        max_cpu_limit_cores: parseFloat(form.maxCpuCores),
        max_memory_limit_kb: parseInt(form.maxMemKb, 10),
      },
    };
    if (
      !manifest.language.slug ||
      !manifest.language.display_name ||
      !manifest.language.version ||
      !manifest.image.reference ||
      !manifest.commands.source_filename ||
      !manifest.commands.run_cmd.length
    ) {
      setFormError("Please fill in all required fields, including at least one run command token.");
      return;
    }
    setSubmitting(true);
    try {
      await adminRegisterLanguage(token, manifest);
      setModalOpen(false);
      await load();
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return onSessionExpired();
      setFormError(e instanceof Error ? e.message : String(e));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section>
      <div className="panel">
        <div className="toolbar">
          <h2>
            Language plugins <span className="count">{languages ? `(${languages.length})` : ""}</span>
          </h2>
          <button className="primary" onClick={openAddModal}>
            + Add Plugin
          </button>
        </div>

        {listError && <div className="banner error">{listError}</div>}
        {!listError && languages && languages.length === 0 && (
          <div className="empty-state">No plugins registered yet — add one to get started.</div>
        )}
        {!listError && languages && languages.length > 0 && (
          <div style={{ overflowX: "auto" }}>
            <table className="data">
              <thead>
                <tr>
                  <th>Language</th>
                  <th>Version</th>
                  <th>Image</th>
                  <th>Default limits</th>
                  <th>Default mem</th>
                  <th>Status</th>
                  <th>Toggle</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {languages.map((l) => (
                  <tr key={l.slug}>
                    <td>
                      <strong>{l.slug}</strong>
                      <div className="muted">{l.display_name}</div>
                    </td>
                    <td className="mono">{l.version}</td>
                    <td className="mono" style={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis" }}>
                      {l.image_ref}
                    </td>
                    <td>
                      {l.default_cpu_time_limit_ms}ms / {l.default_cpu_limit_cores} core
                    </td>
                    <td>{(l.default_memory_limit_kb / 1024).toFixed(0)} MB</td>
                    <td>
                      <span className={"badge " + (l.is_active ? "on" : "off")}>
                        {l.is_active ? "Active" : "Inactive"}
                      </span>
                    </td>
                    <td>
                      <label className="switch">
                        <input
                          type="checkbox"
                          checked={l.is_active}
                          disabled={busySlug === l.slug}
                          onChange={() => toggleActive(l)}
                        />
                        <span className="slider"></span>
                      </label>
                    </td>
                    <td>
                      <button className="danger" disabled={busySlug === l.slug} onClick={() => removePlugin(l)}>
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

      {modalOpen && (
        <Modal
          title="Add language plugin"
          onClose={() => setModalOpen(false)}
          footer={
            <>
              <button className="link" onClick={() => setModalOpen(false)}>
                Cancel
              </button>
              <button className="primary" disabled={submitting} onClick={submitPlugin}>
                {submitting ? "Registering…" : "Register plugin"}
              </button>
            </>
          }
        >
          {formError && <div className="banner error">{formError}</div>}
          <div className="form-section">
            <h4>Start from an existing plugin</h4>
            <div className="form-row single">
              <div>
                <label className="field">Pre-built template</label>
                <select value={selectedTemplate} onChange={(e) => onTemplateChange(e.target.value)}>
                  <option value="">— Start from scratch —</option>
                  {templates.map((t) => (
                    <option key={t.slug} value={t.slug}>
                      {t.display_name} ({t.version})
                    </option>
                  ))}
                </select>
                <div className="hint">
                  Populates every field below from a ready-made manifest for an image already published on Docker
                  Hub. You can still edit anything before registering.
                </div>
              </div>
            </div>
          </div>

          <div className="form-section">
            <h4>Language</h4>
            <div className="form-row triple">
              <div>
                <label className="field">Slug</label>
                <input
                  type="text"
                  placeholder="python3"
                  value={form.slug}
                  onChange={(e) => setForm({ ...form, slug: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Display name</label>
                <input
                  type="text"
                  placeholder="Python 3.11"
                  value={form.displayName}
                  onChange={(e) => setForm({ ...form, displayName: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Version</label>
                <input
                  type="text"
                  placeholder="3.11.4"
                  value={form.version}
                  onChange={(e) => setForm({ ...form, version: e.target.value })}
                />
              </div>
            </div>
          </div>

          <div className="form-section">
            <h4>Image</h4>
            <div className="form-row single">
              <div>
                <label className="field">Image reference</label>
                <input
                  type="text"
                  placeholder="docker.io/library/python:3.11-slim"
                  value={form.image}
                  onChange={(e) => setForm({ ...form, image: e.target.value })}
                />
                <div className="hint">
                  A registry-pullable image, or a locally built tag if this host has skopeo docker-daemon access.
                </div>
              </div>
            </div>
          </div>

          <div className="form-section">
            <h4>Commands</h4>
            <div className="form-row single">
              <div>
                <label className="field">Compile command (argv) — leave empty for interpreted languages</label>
                <ChipInput values={form.compileCmd} onChange={(v) => setForm({ ...form, compileCmd: v })} />
              </div>
            </div>
            <div className="form-row single">
              <div>
                <label className="field">Run command (argv)</label>
                <ChipInput values={form.runCmd} onChange={(v) => setForm({ ...form, runCmd: v })} />
              </div>
            </div>
            <div className="form-row">
              <div>
                <label className="field">Source filename</label>
                <input
                  type="text"
                  placeholder="main.py"
                  value={form.sourceFilename}
                  onChange={(e) => setForm({ ...form, sourceFilename: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Compile timeout (ms)</label>
                <input
                  type="number"
                  value={form.compileTimeoutMs}
                  onChange={(e) => setForm({ ...form, compileTimeoutMs: e.target.value })}
                />
              </div>
            </div>
          </div>

          <div className="form-section">
            <h4>Limits</h4>
            <div className="form-row triple">
              <div>
                <label className="field">Default CPU time (ms)</label>
                <input
                  type="number"
                  value={form.defCpuMs}
                  onChange={(e) => setForm({ ...form, defCpuMs: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Default CPU cores</label>
                <input
                  type="number"
                  step="0.1"
                  value={form.defCpuCores}
                  onChange={(e) => setForm({ ...form, defCpuCores: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Default memory (KB)</label>
                <input
                  type="number"
                  value={form.defMemKb}
                  onChange={(e) => setForm({ ...form, defMemKb: e.target.value })}
                />
              </div>
            </div>
            <div className="form-row triple">
              <div>
                <label className="field">Max CPU time (ms)</label>
                <input
                  type="number"
                  value={form.maxCpuMs}
                  onChange={(e) => setForm({ ...form, maxCpuMs: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Max CPU cores</label>
                <input
                  type="number"
                  step="0.1"
                  value={form.maxCpuCores}
                  onChange={(e) => setForm({ ...form, maxCpuCores: e.target.value })}
                />
              </div>
              <div>
                <label className="field">Max memory (KB)</label>
                <input
                  type="number"
                  value={form.maxMemKb}
                  onChange={(e) => setForm({ ...form, maxMemKb: e.target.value })}
                />
              </div>
            </div>
          </div>
        </Modal>
      )}
    </section>
  );
}
