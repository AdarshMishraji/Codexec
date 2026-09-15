import type {
  AdminLanguage,
  ApiErrorBody,
  CreateApiKeyResponse,
  PluginManifest,
  PluginTemplateSummary,
  PublicApiKey,
  PublicLanguage,
  StatsResponse,
  SubmissionResponse,
  SubmitRequest,
  SubmitResponse,
} from "./types";

/** Thrown by every request() call on a non-2xx response. */
export class ApiError extends Error {
  status: number;
  code: string | undefined;

  constructor(status: number, message: string, code?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

async function request<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const res = await fetch(path, {
    ...opts,
    headers: { "Content-Type": "application/json", ...(opts.headers ?? {}) },
  });

  if (!res.ok) {
    // Most errors are the standard {error, message} envelope, but a
    // missing/invalid Content-Type or malformed JSON body gets a
    // framework-level plain-text response instead (see /docs) - handle
    // both rather than assuming JSON.
    let message = `Request failed (${res.status})`;
    let code: string | undefined;
    try {
      const body = (await res.json()) as ApiErrorBody;
      message = body.message ?? message;
      code = body.error;
    } catch {
      const text = await res.text().catch(() => "");
      if (text) message = text;
    }
    throw new ApiError(res.status, message, code);
  }

  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

// ---- Public, unauthenticated ----

export const getStats = () => request<StatsResponse>("/stats");
export const listLanguagesPublic = () => request<PublicLanguage[]>("/languages");

// ---- Admin (Bearer admin token) ----

function adminHeaders(token: string): HeadersInit {
  return { Authorization: `Bearer ${token}` };
}

export const adminListLanguages = (token: string) =>
  request<AdminLanguage[]>("/admin/languages", { headers: adminHeaders(token) });

export const adminRegisterLanguage = (token: string, manifest: PluginManifest) =>
  request<AdminLanguage>("/admin/languages", {
    method: "POST",
    headers: adminHeaders(token),
    body: JSON.stringify(manifest),
  });

export const adminActivateLanguage = (token: string, slug: string) =>
  request<AdminLanguage>(`/admin/languages/${slug}/activate`, {
    method: "POST",
    headers: adminHeaders(token),
  });

export const adminDeactivateLanguage = (token: string, slug: string) =>
  request<AdminLanguage>(`/admin/languages/${slug}/deactivate`, {
    method: "POST",
    headers: adminHeaders(token),
  });

export const adminDeleteLanguage = (token: string, slug: string) =>
  request<void>(`/admin/languages/${slug}`, { method: "DELETE", headers: adminHeaders(token) });

export const adminListPluginTemplates = (token: string) =>
  request<PluginTemplateSummary[]>("/admin/plugin-templates", { headers: adminHeaders(token) });

export const adminGetPluginTemplate = (token: string, slug: string) =>
  request<PluginManifest>(`/admin/plugin-templates/${slug}`, { headers: adminHeaders(token) });

export const adminListApiKeys = (token: string) =>
  request<PublicApiKey[]>("/admin/api-keys", { headers: adminHeaders(token) });

export const adminCreateApiKey = (token: string, label: string) =>
  request<CreateApiKeyResponse>("/admin/api-keys", {
    method: "POST",
    headers: adminHeaders(token),
    body: JSON.stringify({ label }),
  });

export const adminDeleteApiKey = (token: string, id: string) =>
  request<void>(`/admin/api-keys/${id}`, { method: "DELETE", headers: adminHeaders(token) });

/** A cheap authenticated call used purely to validate a candidate admin token. */
export const verifyAdminToken = (token: string) => adminListLanguages(token);

// ---- Submissions (Bearer API key) ----

function apiKeyHeaders(apiKey: string): HeadersInit {
  return { Authorization: `Bearer ${apiKey}` };
}

export const createSubmission = (apiKey: string, body: SubmitRequest) =>
  request<SubmitResponse>("/submissions", {
    method: "POST",
    headers: apiKeyHeaders(apiKey),
    body: JSON.stringify(body),
  });

export const getSubmission = (apiKey: string, id: string) =>
  request<SubmissionResponse>(`/submissions/${id}`, { headers: apiKeyHeaders(apiKey) });
