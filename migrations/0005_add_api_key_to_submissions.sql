-- Nullable: historical rows predate API key enforcement, and a revoked
-- key's submissions must survive its deletion (ON DELETE SET NULL) rather
-- than losing submission history. Newly-created submissions always have
-- one populated, since the API requires a valid active key to submit.
ALTER TABLE submissions ADD COLUMN api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL;

CREATE INDEX idx_submissions_api_key_id ON submissions (api_key_id);
