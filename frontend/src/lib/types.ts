// Mirrors of codexec-api's JSON response/request shapes. See /docs (Docs.tsx)
// for the authoritative field-by-field reference - these types just need to
// stay in sync with it, not the other way around.

export type SubmissionStatus =
  | "queued"
  | "processing"
  | "completed"
  | "compile_error"
  | "runtime_error"
  | "time_limit_exceeded"
  | "memory_limit_exceeded"
  | "internal_error";

export type SubmissionVerdict = "accepted" | "wrong_answer";

export type LimitExceeded = "cpu_time" | "memory";

/** GET /languages - public, active only. */
export interface PublicLanguage {
  slug: string;
  display_name: string;
  version: string;
  default_cpu_time_limit_ms: number;
  default_cpu_limit_cores: number;
  default_memory_limit_kb: number;
  max_cpu_time_limit_ms: number;
  max_cpu_limit_cores: number;
  max_memory_limit_kb: number;
}

/** GET/POST /admin/languages - full detail, active and inactive. */
export interface AdminLanguage extends PublicLanguage {
  id: string;
  image_ref: string;
  compile_cmd: string[] | null;
  run_cmd: string[];
  source_filename: string;
  compile_time_limit_ms: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface PublicApiKey {
  id: string;
  label: string;
  key_prefix: string;
  is_active: boolean;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface CreateApiKeyResponse extends PublicApiKey {
  /** Shown once, at creation, never retrievable again. */
  api_key: string;
}

export interface PluginTemplateSummary {
  slug: string;
  display_name: string;
  version: string;
}

/** The exact shape POST /admin/languages expects as its body. */
export interface PluginManifest {
  language: { slug: string; display_name: string; version: string };
  image: { reference: string };
  commands: {
    compile_cmd: string[];
    run_cmd: string[];
    source_filename: string;
    compile_time_limit_ms: number;
  };
  limits: {
    default_cpu_time_limit_ms: number;
    default_cpu_limit_cores: number;
    default_memory_limit_kb: number;
    max_cpu_time_limit_ms: number;
    max_cpu_limit_cores: number;
    max_memory_limit_kb: number;
  };
}

export interface SubmitRequest {
  language: string;
  source_code: string;
  stdin?: string;
  expected_output?: string | null;
  cpu_time_limit_ms?: number;
  cpu_limit_cores?: number;
  memory_limit_kb?: number;
}

export interface SubmitResponse {
  id: string;
  status: "queued";
  submitted_at: string;
}

export interface SubmissionResponse {
  id: string;
  language: string;
  status: SubmissionStatus;
  verdict: SubmissionVerdict | null;
  stdout: string | null;
  stderr: string | null;
  compile_output: string | null;
  exit_code: number | null;
  cpu_time_limit_ms: number;
  cpu_limit_cores: number;
  memory_limit_kb: number;
  cpu_time_used_ms: number | null;
  memory_used_kb: number | null;
  limit_exceeded: LimitExceeded | null;
  error_message: string | null;
  submitted_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface StatsResponse {
  total_submissions: number;
  completed_submissions: number;
  success_rate_pct: number;
  submissions_last_24h: number;
  submissions_last_7d: number;
  avg_cpu_time_ms: number | null;
  avg_memory_kb: number | null;
  p50_cpu_time_ms: number | null;
  p95_cpu_time_ms: number | null;
  avg_wall_time_ms: number | null;
  avg_queue_wait_ms: number | null;
  languages_total: number;
  languages_active: number;
  api_keys_total: number;
  api_keys_active: number;
  status_breakdown: { status: SubmissionStatus; count: number }[];
  verdict_breakdown: { verdict: SubmissionVerdict; count: number }[];
  language_breakdown: { slug: string; display_name: string; count: number }[];
  daily_trend: { day: string; count: number }[];
  recent_submissions: {
    id: string;
    language_slug: string;
    status: SubmissionStatus;
    verdict: SubmissionVerdict | null;
    cpu_time_used_ms: number | null;
    memory_used_kb: number | null;
    submitted_at: string;
    finished_at: string | null;
  }[];
}

export interface ApiErrorBody {
  error: string;
  message: string;
}
