-- API tokens for machine clients on /api and /mcp (NFR-2). Only the SHA-256
-- of a token is stored. Revoked tokens are kept, marked by revoked_at.

CREATE TABLE api_token (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name         text NOT NULL CHECK (btrim(name) <> ''),
    scope        token_scope NOT NULL,
    token_hash   bytea NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    revoked_at   timestamptz,
    CHECK (revoked_at >= created_at)
);
