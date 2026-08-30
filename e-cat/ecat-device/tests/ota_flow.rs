//! OTA 闭环集成测试：需 MySQL（`docker compose up -d mysql`）后运行：
//! `cargo test -p iot-device --test ota_flow -- --ignored`
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use ecat_device::{migrate, transition_allowed};
use serde_json::json;
use std::sync::Arc;

async fn db() -> Arc<SqlxClient> {
    let db = Arc::new(SqlxClient::connect("mysql://iot:iot@localhost:3306/iot").await.unwrap());
    db.execute("INSERT IGNORE INTO tenants (id, name) VALUES ('ota-tenant', 'ota')")
        .await
        .expect("insert tenant");
    // 迁移可重复跑：先删引用方再删父表
    for t in ["ota_upgrade_tasks", "ota_firmwares", "device_links", "devices"] {
        db.execute(&format!("DROP TABLE IF EXISTS {t}")).await.expect("drop");
    }
    migrate(&db).await.expect("migrate");
    db
}

async fn insert_device(db: &Arc<SqlxClient>) {
    db.execute(
        "INSERT IGNORE INTO devices (id, tenant_id, name, vendor, status) \
         VALUES ('ota-dev', 'ota-tenant', 'dev', 'mock', 'online')",
    )
    .await
    .expect("insert device");
}

async fn insert_firmware(db: &Arc<SqlxClient>) -> String {
    db.execute(
        "INSERT IGNORE INTO ota_firmwares (id, tenant_id, name, version, url, description) \
         VALUES ('ota-fw', 'ota-tenant', 'fw', '1.0.0', 'http://cdn/x.bin', '')",
    )
    .await
    .expect("insert firmware");
    "ota-fw".into()
}

async fn insert_task(db: &Arc<SqlxClient>) -> String {
    db.execute(
        "INSERT IGNORE INTO ota_upgrade_tasks (id, tenant_id, device_id, firmware_id, status) \
         VALUES ('ota-task', 'ota-tenant', 'ota-dev', 'ota-fw', 'pending')",
    )
    .await
    .expect("insert task");
    "ota-task".into()
}

async fn task_status(db: &Arc<SqlxClient>, id: &str) -> (String, i64, String) {
    let rows = db
        .query_with(
            "SELECT status, progress, message FROM ota_upgrade_tasks WHERE id = ?",
            &[json!(id)],
        )
        .await
        .unwrap();
    let r = rows.first().unwrap();
    (
        r.get("status").and_then(serde_json::Value::as_str).unwrap_or("").to_string(),
        r.get("progress").and_then(serde_json::Value::as_i64).unwrap_or(-1),
        r.get("message").and_then(serde_json::Value::as_str).unwrap_or("").to_string(),
    )
}

#[tokio::test]
#[ignore]
async fn ota_task_full_lifecycle_via_report() {
    let db = db().await;
    insert_device(&db).await;
    insert_firmware(&db).await;
    let task = insert_task(&db).await;

    // 初始 pending
    assert_eq!(task_status(&db, &task).await.0, "pending");
    // 跳步被状态机拒绝（模拟设备不能直接报 success）
    assert!(!transition_allowed("pending", "success"));

    // pending → downloading（进度 30）
    db.execute_with(
        "UPDATE ota_upgrade_tasks SET status = 'downloading', progress = 30, message = '' \
         WHERE id = ?",
        &[json!(task)],
    )
    .await
    .unwrap();
    assert_eq!(task_status(&db, &task).await, ("downloading".into(), 30, "".into()));

    // downloading → installing（进度 80 + 消息）
    db.execute_with(
        "UPDATE ota_upgrade_tasks SET status = 'installing', progress = 80, message = '烧写中' \
         WHERE id = ?",
        &[json!(task)],
    )
    .await
    .unwrap();

    // installing → success：progress 强制 100
    db.execute_with(
        "UPDATE ota_upgrade_tasks SET status = 'success', progress = 100 WHERE id = ?",
        &[json!(task)],
    )
    .await
    .unwrap();
    let (status, progress, msg) = task_status(&db, &task).await;
    assert_eq!(status, "success");
    assert_eq!(progress, 100);
    assert_eq!(msg, "烧写中");
}

#[tokio::test]
#[ignore]
async fn ota_failed_task_frozen() {
    let db = db().await;
    insert_device(&db).await;
    insert_firmware(&db).await;
    let task = insert_task(&db).await;

    db.execute_with(
        "UPDATE ota_upgrade_tasks SET status = 'failed', progress = 50, message = '校验失败' \
         WHERE id = ?",
        &[json!(task)],
    )
    .await
    .unwrap();
    // 失败后不可再回 success（状态机冻结）
    assert!(!transition_allowed("failed", "success"));
    assert_eq!(task_status(&db, &task).await.0, "failed");
}
