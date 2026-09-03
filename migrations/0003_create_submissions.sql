CREATE TABLE submissions (
    id                    UUID PRIMARY KEY,
    language_id           UUID NOT NULL REFERENCES languages(id),
    language_slug         TEXT NOT NULL,
    source_code           TEXT NOT NULL,
    stdin                 TEXT NOT NULL DEFAULT '',
    expected_output       TEXT,
    cpu_time_limit_ms     INTEGER NOT NULL,
    cpu_limit_cores       DOUBLE PRECISION NOT NULL,
    memory_limit_kb       BIGINT  NOT NULL,
    status                submission_status NOT NULL DEFAULT 'queued',
    verdict               submission_verdict,

    exit_code             INTEGER,
    stdout                TEXT,
    stderr                TEXT,
    compile_output        TEXT,
    cpu_time_used_ms      DOUBLE PRECISION,
    memory_used_kb        BIGINT,
    error_message         TEXT,

    worker_id             TEXT,
    lease_expires_at      TIMESTAMPTZ,

    submitted_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at            TIMESTAMPTZ,
    finished_at           TIMESTAMPTZ,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_submissions_status ON submissions (status);
CREATE INDEX idx_submissions_language_id ON submissions (language_id);
CREATE INDEX idx_submissions_submitted_at ON submissions (submitted_at DESC);
