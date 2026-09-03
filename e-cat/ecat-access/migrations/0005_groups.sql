CREATE TABLE IF NOT EXISTS device_groups (
    id BIGINT PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    name VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_group_tenant_name (tenant_id, name),
    INDEX idx_groups_tenant (tenant_id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS device_group_members (
    group_id BIGINT NOT NULL,
    device_id BIGINT NOT NULL,
    PRIMARY KEY (group_id, device_id),
    INDEX idx_member_device (device_id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS device_tags (
    device_id BIGINT NOT NULL,
    tag VARCHAR(32) NOT NULL,
    PRIMARY KEY (device_id, tag),
    INDEX idx_tags_tag (tag)
) ENGINE = InnoDB;
