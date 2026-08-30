use ecat_data_service::ingest::{TOPIC_EVENTS, batch_sql};
use ecat_data_service::models::EventMessage;
use serde_json::json;

#[test]
fn topic_matches_p1_events_bus() {
    // 唯一真源：iot-access/src/events.rs 的 TOPIC_EVENTS
    assert_eq!(TOPIC_EVENTS, "iot.events");
}

#[test]
fn batch_sql_joins_inserts_with_newlines() {
    let evs = vec![
        EventMessage {
            device_id: "d1".into(),
            tenant_id: "t1".into(),
            kind: "property".into(),
            code: "temp".into(),
            value: json!(23.5),
            ts: 1690000000000,
        },
        EventMessage {
            device_id: "d1".into(),
            tenant_id: "t1".into(),
            kind: "property".into(),
            code: "switch".into(),
            value: json!(true),
            ts: 1690000000001,
        },
    ];
    let sql = batch_sql(&evs);
    let lines: Vec<&str> = sql.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("USING iot.devdata"));
    assert!(lines[0].contains("TAGS ('t1', 'd1', 'temp')"));
    assert!(lines[0].contains("VALUES (1690000000000, 23.5, NULL)"));
    // 布尔值走 value_str 列
    assert!(lines[1].contains("'true'"));
}

#[test]
fn batch_sql_escapes_tags() {
    let ev = EventMessage {
        device_id: "d'1".into(),
        tenant_id: "t\\1".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!("hot").into(),
        ts: 1,
    };
    let sql = batch_sql(&[ev]);
    assert!(sql.contains("'t\\\\1'"), "反斜杠未转义: {sql}");
    assert!(sql.contains("'d\\'1'"), "单引号未转义: {sql}");
    assert!(sql.contains("'\"hot\"'"), "JSON 字符串值应序列化为文本: {sql}");
}
