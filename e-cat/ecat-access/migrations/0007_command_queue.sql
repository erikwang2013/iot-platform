CREATE TABLE IF NOT EXISTS command_queue (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    device_id VARCHAR(36) NOT NULL,
    code VARCHAR(64) NOT NULL,
    value_json JSON NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NULL,
    INDEX idx_cq_device (device_id, created_at)
) ENGINE = InnoDB;
