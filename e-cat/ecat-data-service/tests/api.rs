use ecat_data_service::api::{HistoryQuery, build_ch_history_sql, build_history_sql};

#[test]
fn raw_query_is_qualified_and_tenant_scoped() {
    let q = HistoryQuery {
        device_id: "d1".into(),
        code: "temp".into(),
        start: 1690000000000,
        end: 1690000003600,
        agg: None,
        interval: None,
        limit: 100,
        offset: 0,
    };
    let sql = build_history_sql("t1", &q);
    assert!(sql.contains("FROM iot.devdata"));
    assert!(sql.contains("tenant_id = 't1'"), "必须按租户过滤: {sql}");
    assert!(sql.contains("device_id = 'd1'"));
    assert!(sql.contains("code = 'temp'"));
    assert!(sql.contains("ts >= 1690000000000 AND ts <= 1690000003600"));
    assert!(sql.contains("ORDER BY ts"));
    assert!(sql.contains("LIMIT 100 OFFSET 0"));
}

#[test]
fn aggregate_query_uses_interval() {
    let q = HistoryQuery {
        device_id: "d1".into(),
        code: "temp".into(),
        start: 1690000000000,
        end: 1690000003600,
        agg: Some("avg".into()),
        interval: Some("5m".into()),
        limit: 100,
        offset: 0,
    };
    let sql = build_history_sql("t1", &q);
    assert!(sql.contains("_wstart AS ts"));
    assert!(sql.contains("AVG(value)"));
    assert!(sql.contains("INTERVAL(5m)"));
}

#[test]
fn query_escapes_tenant_and_filters() {
    let q = HistoryQuery {
        device_id: "d'1".into(),
        code: "te\"mp".into(),
        start: 0,
        end: 1,
        agg: None,
        interval: None,
        limit: 10,
        offset: 0,
    };
    let sql = build_history_sql("t'1", &q);
    assert!(sql.contains("tenant_id = 't\\'1'"));
    assert!(sql.contains("device_id = 'd\\'1'"));
    assert!(sql.contains("code = 'te\"mp'"));
}

#[test]
fn ch_raw_query_is_qualified_tenant_scoped_and_final() {
    let q = HistoryQuery {
        device_id: "d1".into(),
        code: "temp".into(),
        start: 1690000000000,
        end: 1690000003600,
        agg: None,
        interval: None,
        limit: 100,
        offset: 0,
    };
    let sql = build_ch_history_sql("t1", &q);
    assert!(sql.contains("FROM iot.devdata FINAL"));
    assert!(sql.contains("tenant_id = 't1'"), "必须按租户过滤: {sql}");
    assert!(sql.contains("device_id = 'd1'"));
    assert!(sql.contains("code = 'temp'"));
    assert!(sql.contains("ts >= 1690000000000 AND ts <= 1690000003600"));
    assert!(sql.contains("ORDER BY ts"));
    assert!(sql.contains("LIMIT 100 OFFSET 0"));
}

#[test]
fn ch_aggregate_query_uses_intdiv_bucket() {
    let q = HistoryQuery {
        device_id: "d1".into(),
        code: "temp".into(),
        start: 1690000000000,
        end: 1690000003600,
        agg: Some("max".into()),
        interval: Some("1h".into()),
        limit: 100,
        offset: 0,
    };
    let sql = build_ch_history_sql("t1", &q);
    // 1h = 3600000ms：桶起点对齐 epoch（等价 TDengine _wstart）
    assert!(sql.contains("intDiv(ts, 3600000) * 3600000 AS ts"));
    assert!(sql.contains("MAX(value) AS value"));
    assert!(sql.contains("GROUP BY ts"));
}

#[test]
fn ch_query_escapes_tenant_and_filters() {
    let q = HistoryQuery {
        device_id: "d'1".into(),
        code: "te\"mp".into(),
        start: 0,
        end: 1,
        agg: None,
        interval: None,
        limit: 10,
        offset: 0,
    };
    let sql = build_ch_history_sql("t'1", &q);
    assert!(sql.contains("tenant_id = 't\\'1'"));
    assert!(sql.contains("device_id = 'd\\'1'"));
    assert!(sql.contains("code = 'te\"mp'"));
}
