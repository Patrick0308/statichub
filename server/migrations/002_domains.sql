-- Domains table
CREATE TABLE domains (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    domain TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending_verification' CHECK(status IN ('pending_verification', 'verified', 'failed')),
    verification_token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified_at TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_domains_project ON domains(project_id);
CREATE INDEX idx_domains_domain ON domains(domain);
CREATE INDEX idx_domains_status ON domains(status);

-- Rollback:
-- DROP TABLE IF EXISTS domains;
