//! 需 docker：`docker compose up -d mysql kafka` 后运行：
//! `cargo test -p iot-rule --test rule_flow -- --ignored`
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use ecat_rule::models::{EventMessage, NewRule};
use ecat_rule::push::PushHub;
use ecat_rule::store::{RuleStore, migrate};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn engine_fires_rule_and_alert_reaches_store_and_hub() {
    let db = Arc::new(SqlxClient::connect("mysql://iot:iot@localhost:3306/iot").await.unwrap());
    // rules/alert_records 外键引用 tenants（iot-access 迁移建表）；独立库先行建好父表
    db.execute(
        "CREATE TABLE IF NOT EXISTS tenants (
            id VARCHAR(36) PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        ) ENGINE = InnoDB",
    )
    .await
    .expect("create tenants");
    // rules.tenant_id 外键指向 tenants.id，插入规则前租户记录必须在库
    db.execute(
        "INSERT IGNORE INTO tenants (id, name) VALUES ('itest-tenant', 'itest')",
    )
    .await
    .expect("insert tenant");
    // 迁移可重复跑（DROP 后再建）
    db.execute("DROP TABLE IF EXISTS alert_records").await.expect("drop alerts");
    db.execute("DROP TABLE IF EXISTS rules").await.expect("drop rules");
    migrate(&db).await.expect("migrate");
    let store = Arc::new(RuleStore::new(db.clone()));
    let hub = PushHub::new();
    let mut rx = hub.subscribe("itest-tenant");

    // 建规则（temp > 30）
    let rule = store
        .insert_rule(
            "itest-tenant",
            &NewRule {
                name: "itest-temp".into(),
                device_id: "itest-dev".into(),
                code: "temp".into(),
                operator: "gt".into(),
                threshold: 30.0,
                webhook_url: None,
                enabled: Some(true),
            },
        )
        .await
        .expect("insert rule");

    // 不匹配事件 → 无告警
    let rules = store.list_rules("itest-tenant").await.unwrap();
    let low = EventMessage {
        device_id: "itest-dev".into(),
        tenant_id: "itest-tenant".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!(25.0),
        ts: 1_690_000_000_000,
    };
    assert!(ecat_rule::engine::evaluate(&low, &rules).is_empty());

    // 匹配事件 → evaluate 命中 → 推送 hub + 落告警记录
    let high = EventMessage {
        device_id: "itest-dev".into(),
        tenant_id: "itest-tenant".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!(35.0),
        ts: 1_690_000_000_001,
    };
    let msgs = ecat_rule::engine::evaluate(&high, &rules);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].rule_id, rule.id);
    hub.publish("itest-tenant", &msgs[0]);
    store.insert_alert(&msgs[0]).await.expect("insert alert");

    let got = rx.try_recv().unwrap();
    assert_eq!(got.value, json!(35.0));

    let alerts = store.list_alerts("itest-tenant", None).await.unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].status, "active");
    assert!(store.ack_alert("itest-tenant", &alerts[0].id).await.unwrap());
    let acked = store
        .list_alerts("itest-tenant", Some("acknowledged"))
        .await
        .unwrap();
    assert_eq!(acked.len(), 1);
}

#[tokio::test]
#[ignore]
async fn ws_handshake_verifies_jwt() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // SAFETY: 测试进程内设置，无并发读 env 的其他线程
    unsafe { std::env::set_var("JWT_SECRET", "dev-secret-key-0123456789abcdefghijklmn") };

    // tower oneshot 缺 hyper OnUpgrade extension，WS extractor 永远 426；
    // 起真实 TCP 服务器做真握手（verify_token 在 on_upgrade 前执行，401/101 在 handler 内判定）
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new()
                .route("/ws", axum::routing::get(ecat_rule::ws::ws_handler))
                .with_state(PushHub::new()),
        )
        .await
        .unwrap();
    });

    async fn handshake(addr: std::net::SocketAddr, uri: &str) -> String {
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        s.write_all(
            format!(
                "GET {uri} HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\n\
                 Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
                 Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let mut buf = [0u8; 256];
        let n = s.read(&mut buf).await.unwrap();
        String::from_utf8_lossy(&buf[..n])
            .lines()
            .next()
            .unwrap_or("")
            .to_string()
    }

    // 坏 token → 401（handler 拒绝，未升级）
    let line = handshake(addr, "/ws?token=garbage").await;
    assert!(line.contains("401"), "bad token line: {line}");

    // 合法 token（sub=itest-tenant，与网关同一 JWT_SECRET）→ 101 升级
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        // 注意：AuthClaims.role 是 Option<String>，必须 Some(...)
        &ecat_auth::AuthClaims {
            sub: "itest-tenant".into(),
            exp: None,
            iat: None,
            role: Some("admin".into()),
            extra: Default::default(),
        },
        &jsonwebtoken::EncodingKey::from_secret(b"dev-secret-key-0123456789abcdefghijklmn"),
    )
    .unwrap();
    let line = handshake(addr, &format!("/ws?token={token}")).await;
    assert!(line.starts_with("HTTP/1.1 101"), "good token line: {line}");
}
