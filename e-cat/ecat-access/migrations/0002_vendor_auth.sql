CREATE TABLE IF NOT EXISTS vendor_credentials (
    id BIGINT PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    vendor VARCHAR(64) NOT NULL,
    config_encrypted TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    key_version INT NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_cred_tenant_vendor (tenant_id, vendor),
    CONSTRAINT fk_cred_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS device_links (
    device_id BIGINT PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    vendor VARCHAR(64) NOT NULL,
    vendor_id VARCHAR(128) NOT NULL,
    vendor_name VARCHAR(255) NOT NULL DEFAULT '',
    category VARCHAR(64) NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_link_vendor_vendorid (vendor, vendor_id),
    CONSTRAINT fk_link_device FOREIGN KEY (device_id) REFERENCES devices(id),
    CONSTRAINT fk_link_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;
