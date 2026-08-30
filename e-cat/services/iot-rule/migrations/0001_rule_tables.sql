CREATE TABLE IF NOT EXISTS rules (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    device_id VARCHAR(64) NOT NULL,
    code VARCHAR(64) NOT NULL,
    operator VARCHAR(8) NOT NULL,
    threshold DOUBLE NOT NULL,
    webhook_url VARCHAR(512) NULL,
    enabled INT NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY idx_rule_tenant (tenant_id),
    CONSTRAINT fk_rule_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS alert_records (
    id VARCHAR(36) PRIMARY KEY,
    rule_id VARCHAR(36) NOT NULL,
    tenant_id VARCHAR(36) NOT NULL,
    device_id VARCHAR(64) NOT NULL,
    code VARCHAR(64) NOT NULL,
    operator VARCHAR(8) NOT NULL,
    threshold DOUBLE NOT NULL,
    value TEXT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY idx_alert_tenant_ts (tenant_id, created_at),
    CONSTRAINT fk_alert_rule FOREIGN KEY (rule_id) REFERENCES rules(id),
    CONSTRAINT fk_alert_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;
