use ecat_data_service::export::{csv_of_points, xlsx_of_points};
use ecat_data_service::models::HistoryPoint;
use serde_json::json;

#[test]
fn csv_has_header_and_escapes_commas() {
    let points = vec![
        HistoryPoint { ts: 1690000000000, value: json!(23.5) },
        HistoryPoint { ts: 1690000000001, value: json!("a,b\"c") },
    ];
    let csv = csv_of_points(&points);
    assert!(csv.starts_with("\u{FEFF}ts,value\n"), "缺表头(含 UTF-8 BOM): {csv}");
    assert!(csv.contains("1690000000000,23.5\n"));
    assert!(csv.contains("\"a,b\"\"c\"\n"), "逗号/引号未转义: {csv}");
}

#[test]
fn xlsx_produces_valid_zip() {
    let points = vec![
        HistoryPoint { ts: 1690000000000, value: json!(23.5) },
        HistoryPoint { ts: 1690000000001, value: json!("hot") },
    ];
    let buf = xlsx_of_points(&points).unwrap();
    // xlsx = zip 容器，魔数 PK\x03\x04
    assert_eq!(&buf[0..4], b"PK\x03\x04");
}
