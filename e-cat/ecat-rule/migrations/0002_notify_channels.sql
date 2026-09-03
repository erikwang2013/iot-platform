CREATE TABLE IF NOT EXISTS notify_channels (
    id BIGINT PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    channel VARCHAR(16) NOT NULL,
    config JSON NOT NULL,
    enabled INT NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_notify_tenant_channel (tenant_id, channel),
    KEY idx_notify_tenant (tenant_id),
    CONSTRAINT fk_notify_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;
