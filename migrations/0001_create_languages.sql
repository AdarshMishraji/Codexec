CREATE TABLE languages (
    id                          UUID PRIMARY KEY,
    slug                        TEXT NOT NULL UNIQUE,
    display_name                TEXT NOT NULL,
    version                     TEXT NOT NULL,
    image_ref                   TEXT NOT NULL,
    compile_cmd                 JSONB,
    run_cmd                     JSONB NOT NULL,
    source_filename             TEXT NOT NULL,
    compile_time_limit_ms       INTEGER NOT NULL DEFAULT 10000,
    default_cpu_time_limit_ms   INTEGER NOT NULL DEFAULT 2000,
    default_cpu_limit_cores     DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    default_memory_limit_kb     BIGINT  NOT NULL DEFAULT 262144,
    max_cpu_time_limit_ms       INTEGER NOT NULL DEFAULT 10000,
    max_cpu_limit_cores         DOUBLE PRECISION NOT NULL DEFAULT 2.0,
    max_memory_limit_kb         BIGINT  NOT NULL DEFAULT 1048576,
    is_active                   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_languages_is_active ON languages (is_active) WHERE is_active;
