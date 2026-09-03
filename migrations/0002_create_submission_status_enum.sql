CREATE TYPE submission_status AS ENUM (
    'queued',
    'processing',
    'completed',
    'compile_error',
    'runtime_error',
    'time_limit_exceeded',
    'memory_limit_exceeded',
    'internal_error'
);

CREATE TYPE submission_verdict AS ENUM ('accepted', 'wrong_answer');
