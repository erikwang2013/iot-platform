//! OpenAPI 3.0 文档导出（A-4）：只读开放端点文档。
//! 挂载于网关公开路径 /api/open/openapi.json，供 swagger-editor / 开发者阅读。
use ecat_openapi::{
    OpenApiBuilder, Schema, schema_ref, string_schema,
};
use std::collections::HashMap;

/// 构建只读端点 OpenAPI 3.0 规范。
pub fn read_only_spec() -> ecat_openapi::OpenApiSpec {
    OpenApiBuilder::new("IoT Platform Open API", "1.0.0")
        // 设备
        .add_route("/devices", "GET", "设备列表", vec!["device".into()])
        .add_route("/devices/stats", "GET", "设备统计（总数/在线/离线/厂商分布）", vec!["device".into()])
        .add_route("/devices/groups", "GET", "设备分组列表", vec!["device".into()])
        .add_route("/devices/{id}/tags", "GET", "设备标签列表", vec!["device".into()])
        // 物模型
        .add_route("/models/things", "GET", "物模型列表", vec!["model".into()])
        .add_route("/models/things/{id}", "GET", "设备物模型（属性/事件/服务）", vec!["model".into()])
        // 历史数据
        .add_route("/data/history", "GET", "历史数据（聚合查询）", vec!["data".into()])
        .add_route("/data/export", "GET", "历史数据导出", vec!["data".into()])
        // 告警
        .add_route("/rule/alerts", "GET", "告警记录列表", vec!["alert".into()])
        .add_route("/rule/stats", "GET", "告警统计（总数/未处理数）", vec!["alert".into()])
        // 租户用量
        .add_route("/tenants", "GET", "租户列表（含设备用量/配额）", vec!["tenant".into()])
        .add_schema("Device", device_schema())
        .add_schema("DeviceStats", device_stats_schema())
        .add_schema("HistoryPoint", history_point_schema())
        .build()
}

fn device_schema() -> HashMap<String, Schema> {
    let mut m = HashMap::new();
    m.insert("id".into(), string_schema());
    m.insert("name".into(), string_schema());
    m.insert("vendor".into(), string_schema());
    m.insert("status".into(), string_schema());
    m
}

fn device_stats_schema() -> HashMap<String, Schema> {
    let mut m = HashMap::new();
    m.insert("total".into(), string_schema());
    m.insert("online".into(), string_schema());
    m.insert("offline".into(), string_schema());
    m.insert("vendors".into(), string_schema());
    m
}

fn history_point_schema() -> HashMap<String, Schema> {
    let mut m = HashMap::new();
    m.insert("ts".into(), string_schema());
    m.insert("value".into(), string_schema());
    m
}

/// 供测试 / 可复用：暴露 schema_ref 便于引用。
pub fn device_ref() -> Schema {
    schema_ref("Device")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_serializes_to_openapi_30() {
        let spec = read_only_spec();
        assert_eq!(spec.openapi, "3.0.3");
        assert_eq!(spec.info.title, "IoT Platform Open API");
        // 包含只读端点
        let json = serde_json::to_value(&spec).unwrap();
        assert!(json.pointer("/paths/~1devices").is_some());
        assert!(json.pointer("/paths/~1devices~1stats").is_some());
        assert!(json.pointer("/paths/~1rule~1alerts").is_some());
        // components 含 schema
        assert!(json.pointer("/components/schemas/Device").is_some());
    }

    #[test]
    fn device_ref_points_to_components() {
        let r = device_ref();
        assert_eq!(r.reference.as_deref(), Some("#/components/schemas/Device"));
    }

    #[test]
    fn json_is_valid_openapi() {
        let spec = read_only_spec();
        let json = serde_json::to_string(&spec).unwrap();
        // 基本结构：openapi + info + paths
        assert!(json.contains("\"openapi\":\"3.0.3\""));
        assert!(json.contains("\"paths\""));
        assert!(json.contains("\"info\""));
    }
}
