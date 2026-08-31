CREATE TABLE IF NOT EXISTS api_keys (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    name VARCHAR(64) NOT NULL,
    secret_hash VARCHAR(64) NOT NULL,
    scope VARCHAR(16) NOT NULL DEFAULT 'read',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TIMESTAMP NULL,
    INDEX idx_keys_tenant (tenant_id)
) ENGINE = InnoDB;
