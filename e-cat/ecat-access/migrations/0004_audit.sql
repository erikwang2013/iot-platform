CREATE TABLE IF NOT EXISTS audit_log (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    tenant_id VARCHAR(64) NOT NULL DEFAULT 'anonymous',
    role VARCHAR(32) NOT NULL DEFAULT '',
    method VARCHAR(8) NOT NULL,
    path VARCHAR(255) NOT NULL,
    status INT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_audit_tenant_created (tenant_id, created_at)
) ENGINE = InnoDB;
