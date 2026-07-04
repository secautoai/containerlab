-- NetPilot persistence schema: identity/RBAC, lab ownership & sharing,
-- firmware (disk image) metadata, and agent-session history.

CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    -- 'admin' (everything), 'operator' (build/run/share own labs),
    -- 'viewer' (read-only).
    role          TEXT NOT NULL DEFAULT 'operator',
    password_hash TEXT NOT NULL,
    disabled      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One row per lab known to the file store; owner + visibility live here.
CREATE TABLE IF NOT EXISTS labs (
    id          UUID PRIMARY KEY,
    owner_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    -- 'private' (owner + explicit shares), 'public' (any authenticated user).
    visibility  TEXT NOT NULL DEFAULT 'private',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Explicit per-user grants on a private lab.
CREATE TABLE IF NOT EXISTS lab_shares (
    lab_id     UUID NOT NULL REFERENCES labs(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- 'view' (read-only) or 'edit' (mutate + start/stop).
    access     TEXT NOT NULL DEFAULT 'view',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (lab_id, user_id)
);

-- Firmware library: metadata for uploaded disk images (bytes live on disk
-- under <data>/images/<template>/<version>/).
CREATE TABLE IF NOT EXISTS firmware_images (
    id           UUID PRIMARY KEY,
    template     TEXT NOT NULL,
    version      TEXT NOT NULL,
    filename     TEXT NOT NULL,
    size_bytes   BIGINT NOT NULL,
    sha256       TEXT,
    uploaded_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (template, version)
);

-- Agent conversations, one per (lab, user) working session, resumable.
CREATE TABLE IF NOT EXISTS agent_sessions (
    id          UUID PRIMARY KEY,
    lab_id      UUID NOT NULL,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title       TEXT NOT NULL DEFAULT 'Agent session',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ordered transcript items (user/assistant/tool/report/error) as JSON.
CREATE TABLE IF NOT EXISTS agent_events (
    id          BIGSERIAL PRIMARY KEY,
    session_id  UUID NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    payload     JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (session_id, seq)
);

-- Append-only audit trail of security-relevant actions.
CREATE TABLE IF NOT EXISTS audit_log (
    id         BIGSERIAL PRIMARY KEY,
    user_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    action     TEXT NOT NULL,
    target     TEXT,
    detail     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_labs_owner ON labs(owner_id);
CREATE INDEX IF NOT EXISTS idx_shares_user ON lab_shares(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_lab_user ON agent_sessions(lab_id, user_id);
CREATE INDEX IF NOT EXISTS idx_events_session ON agent_events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_log(user_id, created_at);
