//! 存量库升级：旧版 uuid 主键 → snowflake BIGINT（与 11 个迁移文件终态一致）。
//!
//! 新库 / 已迁移库天然安全：闸门只认 uuid 格式（REGEXP），数字 BIGINT 永不匹配 → no-op 退出 0。
//! 铸号复用 `ecat::ids::next_id()`：算法/epoch/worker 布局与运行时代码同源，无漂移。
//! 悬空引用（uuid 值无对应父行）一律中止并列出，不代为造号。
//!
//! 前置（工具只打印提醒，不代执行）：四个服务已停止；已 mysqldump 备份；
//! DATABASE_URL 指向目标库（与 iot-access 同一连接串）。
//! 单次可中断续跑：持久映射表 `_legacy_uuid_map` 为幂等来源，重跑跳过已映射行。
//! 已知缺口（工具只管 MySQL）：CH/OpenSearch/Redis shadow 中旧 uuid 历史键、
//! 磁盘 {old_uuid}.bin OTA 固件文件不迁移，处置见 docs/deploy/upgrade-legacy-ids.md。

use ecat::ids::next_id;
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde_json::{Value, json};
use std::collections::HashSet;

/// uuid v4 形态（DB 旧值统一为简单 uuid）。
const UUID_RE: &str = "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-";

/// 主键被转换的 15 张父表（id VARCHAR(36) PRIMARY KEY → BIGINT）。
const PARENT_IDS: &[&str] = &[
    "devices",
    "vendor_credentials",
    "users",
    "thing_models",
    "ota_firmwares",
    "ota_upgrade_tasks",
    "device_groups",
    "api_keys",
    "command_queue",
    "rules",
    "alert_records",
    "notify_channels",
    "daily_reports",
    "cdn_providers",
    "cdn_tasks",
];

/// 值改写域列：(表, 列, 对应父表)。uuid 值必命中父映射；NULL/外部标识天然跳过。
const REWRITE: &[(&str, &str, &str)] = &[
    ("device_links", "device_id", "devices"),
    ("thing_models", "device_id", "devices"),
    ("ota_upgrade_tasks", "device_id", "devices"),
    ("command_queue", "device_id", "devices"),
    ("device_group_members", "device_id", "devices"),
    ("device_tags", "device_id", "devices"),
    ("rules", "device_id", "devices"),
    ("rules", "action_device_id", "devices"),
    ("alert_records", "device_id", "devices"),
    ("ota_upgrade_tasks", "firmware_id", "ota_firmwares"),
    ("alert_records", "rule_id", "rules"),
    ("cdn_tasks", "provider_id", "cdn_providers"),
];

/// 列型改造：(表, 列, 终态 DDL)。与迁移文件终态一致（NULL 性照抄）；
/// rules.action_device_id 旧库可能缺失（legacy ALTER 分支），存在才改型。
const MODIFY_COLS: &[(&str, &str, &str)] = &[
    ("device_links", "device_id", "BIGINT NOT NULL"),
    ("thing_models", "device_id", "BIGINT NULL"),
    ("ota_upgrade_tasks", "device_id", "BIGINT NOT NULL"),
    ("ota_upgrade_tasks", "firmware_id", "BIGINT NOT NULL"),
    ("command_queue", "device_id", "BIGINT NOT NULL"),
    ("device_group_members", "group_id", "BIGINT NOT NULL"),
    ("device_group_members", "device_id", "BIGINT NOT NULL"),
    ("device_tags", "device_id", "BIGINT NOT NULL"),
    ("rules", "device_id", "BIGINT NOT NULL"),
    ("rules", "action_device_id", "BIGINT NULL"),
    ("alert_records", "device_id", "BIGINT NOT NULL"),
    ("alert_records", "rule_id", "BIGINT NOT NULL"),
    ("cdn_tasks", "provider_id", "BIGINT NOT NULL"),
];

/// 引用转换主键的 FK：(表, 约束名, 重建 DDL 尾部)。改型前删、完成后按原样重建。
/// 定义文本内联自当前迁移文件（fk_link_device / fk_alert_rule / fk_cdn_task_provider）。
const FKS: &[(&str, &str, &str)] = &[
    ("device_links", "fk_link_device", "FOREIGN KEY (device_id) REFERENCES devices(id)"),
    ("alert_records", "fk_alert_rule", "FOREIGN KEY (rule_id) REFERENCES rules(id)"),
    (
        "cdn_tasks",
        "fk_cdn_task_provider",
        "FOREIGN KEY (provider_id) REFERENCES cdn_providers(id) ON DELETE CASCADE",
    ),
];

/// 无 FK 保护的域列（uuid 值需在父映射中，否则即悬空）——逐列做孤儿检查。
const ORPHAN_CHECK: &[(&str, &str)] = &[
    ("thing_models", "device_id"),
    ("ota_upgrade_tasks", "device_id"),
    ("ota_upgrade_tasks", "firmware_id"),
    ("command_queue", "device_id"),
    ("device_group_members", "device_id"),
    ("device_tags", "device_id"),
    ("rules", "device_id"),
    ("rules", "action_device_id"),
    ("alert_records", "device_id"),
];

fn err(msg: &str) -> ! {
    eprintln!("remap-legacy-ids: {msg}");
    std::process::exit(1);
}

async fn query_count(db: &SqlxClient, sql: &str) -> Result<i64, String> {
    let rows = db.query_with(sql, &[]).await.map_err(|e| e.to_string())?;
    Ok(rows
        .first()
        .and_then(|r| r.get("n"))
        .and_then(Value::as_i64)
        .unwrap_or(0))
}

/// 收集 uuid 格式 id 行。map_ids：父表集合（已存在映射的 uuid）。
async fn collect_uuids(
    db: &SqlxClient,
    table: &str,
    col: &str,
) -> Result<Vec<String>, String> {
    let sql = format!("SELECT {col} AS v FROM {table} WHERE {col} REGEXP '{UUID_RE}'");
    let rows = db.query_with(&sql, &[]).await.map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .filter_map(|r| r.get("v").and_then(Value::as_str).map(str::to_string))
        .collect())
}

async fn column_exists(db: &SqlxClient, table: &str, col: &str) -> Result<bool, String> {
    let rows = db
        .query_with(
            "SELECT COUNT(*) AS n FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND COLUMN_NAME = ?",
            &[json!(table), json!(col)],
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .first()
        .and_then(|r| r.get("n"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        > 0)
}

/// 老库可能缺新版本才有的表（如 cdn_*/notify_channels 等）——缺表=无数据可迁，跳过而非报错。
async fn table_exists(db: &SqlxClient, table: &str) -> Result<bool, String> {
    let rows = db
        .query_with(
            "SELECT COUNT(*) AS n FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?",
            &[json!(table)],
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .first()
        .and_then(|r| r.get("n"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        > 0)
}

/// FK 存在性（中断续跑语义：FK 删后若后续失败，重跑不得对已删 FK 重复 DROP）。
async fn fk_exists(db: &SqlxClient, table: &str, name: &str) -> Result<bool, String> {
    let rows = db
        .query_with(
            "SELECT COUNT(*) AS n FROM information_schema.TABLE_CONSTRAINTS \
             WHERE CONSTRAINT_SCHEMA = DATABASE() AND TABLE_NAME = ? \
               AND CONSTRAINT_NAME = ? AND CONSTRAINT_TYPE = 'FOREIGN KEY'",
            &[json!(table), json!(name)],
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .first()
        .and_then(|r| r.get("n"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        > 0)
}

/// 目标列是否仍有非 bigint（中断收尾判定）：任何存在的父表 id 或域列未改型即 true。
async fn needs_type_fix(db: &SqlxClient) -> Result<bool, String> {
    let mut cols = Vec::new();
    for t in PARENT_IDS {
        cols.push(((*t).to_string(), "id".to_string()));
    }
    for (t, c, _) in MODIFY_COLS {
        cols.push(((*t).to_string(), (*c).to_string()));
    }
    for (t, c) in cols {
        if !column_exists(db, &t, &c).await? {
            continue;
        }
        let rows = db
            .query_with(
                "SELECT DATA_TYPE FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND COLUMN_NAME = ?",
                &[json!(t), json!(c)],
            )
            .await
            .map_err(|e| e.to_string())?;
        let dt = rows
            .first()
            .and_then(|r| r.get("DATA_TYPE"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if dt != "bigint" {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn run(db: &SqlxClient) -> Result<(), String> {
    println!("== remap-legacy-ids 前置提醒：服务须已停止、库须已 mysqldump 备份 ==");

    // ---- 幂等闸门：所有父表无 uuid 行即 no-op ----
    let mut todo = 0i64;
    for t in PARENT_IDS {
        if !table_exists(db, t).await? {
            continue; // 老库缺表（版本差异），无数据可迁
        }
        todo += query_count(
            db,
            &format!("SELECT COUNT(*) AS n FROM {t} WHERE id REGEXP '{UUID_RE}'"),
        )
        .await?;
    }
    if todo == 0 {
        // 数据无 uuid 不代表迁移完成：中断可能停在改型阶段（列仍 VARCHAR）。
        // 此时若列型已全 bigint 才是真 no-op；否则继续走改型/校验收尾。
        if !needs_type_fix(db).await? {
            println!("no legacy uuid rows, nothing to do (新库或已迁移库)");
            return Ok(());
        }
        println!("无 uuid 数据残留，但列型未收敛（上次中断于改型阶段），继续收尾…");
    } else {
        println!("发现 {todo} 行 uuid 主键，开始迁移…");
    }

    // ---- 行数基准（校验用）----
    let mut tables = HashSet::new();
    for (t, _, _) in REWRITE {
        tables.insert(t.to_string());
    }
    for t in PARENT_IDS {
        tables.insert((*t).to_string());
    }
    let mut before = Vec::new();
    for t in &tables {
        if !table_exists(db, t).await? {
            continue;
        }
        before.push((
            t.clone(),
            query_count(db, &format!("SELECT COUNT(*) AS n FROM {t}")).await?,
        ));
    }

    // ---- 持久映射表（IF NOT EXISTS：中断续跑来源）----
    db.execute_with(
        "CREATE TABLE IF NOT EXISTS _legacy_uuid_map \
         (old_id VARCHAR(36) PRIMARY KEY, new_id BIGINT NOT NULL UNIQUE) ENGINE = InnoDB",
        &[],
    )
    .await
    .map_err(|e| format!("create map table: {e}"))?;

    // ---- 为每个父表的 uuid 行分配 new_id（服务已停，无并发铸号）----
    let rows = db
        .query_with("SELECT old_id FROM _legacy_uuid_map", &[])
        .await
        .map_err(|e| e.to_string())?;
    let mapped: HashSet<String> = rows
        .iter()
        .filter_map(|r| r.get("old_id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let mut total_mapped = 0u64;
    for t in PARENT_IDS {
        if !table_exists(db, t).await? {
            continue; // 老库缺表，跳过
        }
        let uuids = collect_uuids(db, t, "id").await?;
        let mut inserted = 0u64;
        for old in uuids {
            if mapped.contains(&old) {
                continue; // 上次中断已映射
            }
            db.execute_with(
                "INSERT INTO _legacy_uuid_map (old_id, new_id) VALUES (?, ?)",
                &[json!(old), json!(next_id())],
            )
            .await
            .map_err(|e| format!("map insert {t}.{old}: {e}"))?;
            inserted += 1;
        }
        if inserted > 0 {
            println!("  mapped {t}: {inserted} 行");
        }
        total_mapped += inserted;
    }

    // ---- 孤儿检查（uuid 值不在父映射 = 悬空，中止）----
    let mut orphans = Vec::new();
    for (t, c) in ORPHAN_CHECK {
        if !column_exists(db, t, c).await? {
            continue; // 旧库缺列（如 action_device_id 走 legacy ALTER 分支）
        }
        let n = query_count(
            db,
            &format!(
                "SELECT COUNT(*) AS n FROM {t} \
                 WHERE {c} REGEXP '{UUID_RE}' AND {c} NOT IN (SELECT old_id FROM _legacy_uuid_map)"
            ),
        )
        .await?;
        if n > 0 {
            orphans.push(format!("{t}.{c}: {n} 行"));
        }
    }
    if !orphans.is_empty() {
        err(&format!(
            "发现悬空引用（uuid 值无对应父行），请先清理再升级：\n  {}",
            orphans.join("\n  ")
        ));
    }

    // ---- 删引用转换主键的 FK（仅删存在的；中断续跑时可能已删 → 跳过）----
    for (t, name, _) in FKS {
        if !table_exists(db, t).await? {
            continue; // 老库缺表，FK 随之不存在
        }
        if !fk_exists(db, t, name).await? {
            continue; // 上次中断已删（或本就不存在）
        }
        db.execute_with(&format!("ALTER TABLE {t} DROP FOREIGN KEY {name}"), &[])
            .await
            .map_err(|e| format!("drop fk {name}: {e}"))?;
        println!("  dropped FK {name}");
    }

    // ---- 值改写（子表域列）----
    for (t, c, _) in REWRITE {
        if !column_exists(db, t, c).await? {
            continue;
        }
        // 无 uuid 值则跳过（列已改型 BIGINT 时 JOIN uuid 映射表会 cast 报错）。
        let left = query_count(
            db,
            &format!("SELECT COUNT(*) AS n FROM {t} WHERE {c} REGEXP '{UUID_RE}'"),
        )
        .await?;
        if left == 0 {
            continue;
        }
        let affected = db
            .execute_with(
                &format!(
                    "UPDATE {t} JOIN _legacy_uuid_map m ON {t}.{c} = m.old_id SET {t}.{c} = m.new_id"
                ),
                &[],
            )
            .await
            .map_err(|e| format!("rewrite {t}.{c}: {e}"))?;
        println!("  rewrote {t}.{c}: {affected} 行");
    }

    // ---- 父表主键改写 ----
    for t in PARENT_IDS {
        if !table_exists(db, t).await? {
            continue;
        }
        // 无 uuid 行的表（从未有 uuid 或上次中断已改型 BIGINT）跳过——
        // 否则 BIGINT 列与映射表 uuid 串 JOIN 比较会触发 cast DOUBLE 报错。
        let left = query_count(
            db,
            &format!("SELECT COUNT(*) AS n FROM {t} WHERE id REGEXP '{UUID_RE}'"),
        )
        .await?;
        if left == 0 {
            continue;
        }
        let affected = db
            .execute_with(
                &format!(
                    "UPDATE {t} JOIN _legacy_uuid_map m ON {t}.id = m.old_id SET {t}.id = m.new_id"
                ),
                &[],
            )
            .await
            .map_err(|e| format!("rewrite {t}.id: {e}"))?;
        if affected > 0 {
            println!("  rewrote {t}.id: {affected} 行");
        }
    }

    // ---- 列型改造：15 张父表 id + 域列 → BIGINT（终态与迁移文件一致）----
    for t in PARENT_IDS {
        if !table_exists(db, t).await? {
            continue;
        }
        db.execute_with(&format!("ALTER TABLE {t} MODIFY id BIGINT NOT NULL"), &[])
            .await
            .map_err(|e| format!("alter {t}.id: {e}"))?;
    }
    for (t, c, ddl) in MODIFY_COLS {
        if !column_exists(db, t, c).await? {
            continue;
        }
        db.execute_with(&format!("ALTER TABLE {t} MODIFY {c} {ddl}"), &[])
            .await
            .map_err(|e| format!("alter {t}.{c}: {e}"))?;
    }

    // ---- 重建 FK：表存在且 FK 缺失即补（收敛到新 schema；中断续跑同样自愈）----
    for (t, name, ddl) in FKS {
        if !table_exists(db, t).await? {
            continue;
        }
        if fk_exists(db, t, name).await? {
            continue; // 未删过或已完成，无需重建
        }
        db.execute_with(&format!("ALTER TABLE {t} ADD CONSTRAINT {name} {ddl}"), &[])
            .await
            .map_err(|e| format!("recreate fk {name}: {e}"))?;
        println!("  recreated FK {name}");
    }

    // ---- 清理映射表 ----
    db.execute_with("DROP TABLE _legacy_uuid_map", &[])
        .await
        .map_err(|e| format!("drop map: {e}"))?;

    // ---- 校验：类型 = bigint / 无 uuid 残留 / 行数不变 ----
    let mut cols = Vec::new();
    for t in PARENT_IDS {
        cols.push(((*t).to_string(), "id".to_string()));
    }
    for (t, c, _) in MODIFY_COLS {
        cols.push(((*t).to_string(), (*c).to_string()));
    }
    for (t, c) in cols {
        if !column_exists(db, &t, &c).await? {
            continue;
        }
        let rows = db
            .query_with(
                "SELECT DATA_TYPE FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND COLUMN_NAME = ?",
                &[json!(t), json!(c)],
            )
            .await
            .map_err(|e| e.to_string())?;
        let dt = rows
            .first()
            .and_then(|r| r.get("DATA_TYPE"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if dt != "bigint" {
            err(&format!("校验失败：{t}.{c} 类型 {dt}，应为 bigint"));
        }
        let left = query_count(
            db,
            &format!("SELECT COUNT(*) AS n FROM {t} WHERE {c} REGEXP '{UUID_RE}'"),
        )
        .await?;
        if left > 0 {
            err(&format!("校验失败：{t}.{c} 残留 {left} 行 uuid"));
        }
    }
    for (t, n0) in before {
        let n1 = query_count(db, &format!("SELECT COUNT(*) AS n FROM {t}")).await?;
        if n0 != n1 {
            err(&format!("校验失败：{t} 行数 {n0} → {n1}"));
        }
    }
    println!("校验通过：类型 bigint、无 uuid 残留、行数不变（{total_mapped} 行映射）。完成。");
    Ok(())
}

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| err("DATABASE_URL 未设置（与 iot-access 相同连接串）"));
    let db = SqlxClient::connect(&db_url)
        .await
        .unwrap_or_else(|e| err(&format!("连接失败：{e}")));
    run(&db).await.unwrap_or_else(|e| err(&e));
}
