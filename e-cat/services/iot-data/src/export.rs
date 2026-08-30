use crate::models::HistoryPoint;
use serde_json::Value;

/// CSV：UTF-8 BOM + ts,value 两列；含逗号/引号/换行的值按 RFC 4180 加引号。
pub fn csv_of_points(points: &[HistoryPoint]) -> String {
    let mut out = String::from("\u{FEFF}ts,value\n");
    for p in points {
        out.push_str(&p.ts.to_string());
        out.push(',');
        out.push_str(&csv_field(&value_text(&p.value)));
        out.push('\n');
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// xlsx：两列（ts 数值毫秒、value 文本），返回 zip 字节。
pub fn xlsx_of_points(points: &[HistoryPoint]) -> Result<Vec<u8>, String> {
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.push_worksheet();
    ws.write_string(0, 0, "ts").map_err(|e| e.to_string())?;
    ws.write_string(0, 1, "value").map_err(|e| e.to_string())?;
    for (i, p) in points.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_number(row, 0, p.ts as f64).map_err(|e| e.to_string())?;
        ws.write_string(row, 1, &value_text(&p.value))
            .map_err(|e| e.to_string())?;
    }
    wb.save_to_buffer().map_err(|e| e.to_string())
}
