CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    username VARCHAR(64) NOT NULL,
    password_hash VARCHAR(64) NOT NULL,
    role VARCHAR(16) NOT NULL DEFAULT 'operator',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_users_username (username),
    CONSTRAINT fk_users_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS thing_models (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    device_id VARCHAR(36) NULL,
    schema_json JSON NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_models_tenant (tenant_id),
    INDEX idx_models_device (device_id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS ota_firmwares (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    version VARCHAR(64) NOT NULL,
    url VARCHAR(512) NOT NULL,
    description VARCHAR(512) NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_firmware_tenant_version (tenant_id, version)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS ota_upgrade_tasks (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    device_id VARCHAR(36) NOT NULL,
    firmware_id VARCHAR(36) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    progress INT NOT NULL DEFAULT 0,
    message VARCHAR(512) NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_ota_tenant (tenant_id)
) ENGINE = InnoDB;
