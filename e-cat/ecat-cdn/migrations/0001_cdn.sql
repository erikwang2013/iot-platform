CREATE TABLE IF NOT EXISTS cdn_providers (
    id BIGINT PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    vendor VARCHAR(64) NOT NULL,
    domain VARCHAR(255) NOT NULL DEFAULT '',
    config_encrypted TEXT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'disabled',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY idx_cdn_provider_tenant (tenant_id),
    CONSTRAINT fk_cdn_provider_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS cdn_tasks (
    id BIGINT PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    provider_id BIGINT NOT NULL,
    kind VARCHAR(16) NOT NULL,
    urls_json TEXT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'pending',
    error TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY idx_cdn_task_tenant_ts (tenant_id, created_at),
    CONSTRAINT fk_cdn_task_provider FOREIGN KEY (provider_id) REFERENCES cdn_providers(id) ON DELETE CASCADE
) ENGINE = InnoDB;
