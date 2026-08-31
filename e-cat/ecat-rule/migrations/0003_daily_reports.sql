CREATE TABLE IF NOT EXISTS daily_reports (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    report_date DATE NOT NULL,
    summary JSON NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_report_tenant_date (tenant_id, report_date),
    CONSTRAINT fk_report_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;
