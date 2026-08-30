use crate::models::{AlertMessage, NotifyChannel};
use lettre::{
    Message, SmtpTransport, Transport,
    message::Mailbox,
    transport::smtp::authentication::Credentials,
    transport::smtp::client::{Tls, TlsParameters},
    transport::smtp::extension::ClientId,
};
use serde_json::Value;

pub const CHANNEL_EMAIL: &str = "email";
pub const CHANNEL_DINGTALK: &str = "dingtalk";
pub const CHANNEL_WECOM: &str = "wecom";

fn cfg_str(c: &Value, key: &str) -> Option<String> {
    c.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// 通知正文（邮件 / 钉钉 / 企微共用同一文案）。
fn alert_text(msg: &AlertMessage) -> String {
    format!(
        "【IoT 告警】{}\n设备：{}\n指标：{} {} {}\n当前值：{}\n时间：{}",
        msg.rule_name, msg.device_id, msg.code, msg.operator, msg.threshold, msg.value, msg.ts
    )
}

fn parse_mailbox(s: &str, what: &str) -> Result<Mailbox, String> {
    s.parse::<Mailbox>().map_err(|e| format!("invalid {what}: {e}"))
}

/// SMTP 发送（阻塞，调用方应放 spawn_blocking）。
/// smtp_tls 配置：starttls（默认，587 常用）| implicit（465）| none（25 明文）。
fn send_email(ch: &NotifyChannel, msg: &AlertMessage) -> Result<(), String> {
    let c = &ch.config;
    let host = cfg_str(c, "smtp_host").ok_or("smtp_host not configured")?;
    let port = c.get("smtp_port").and_then(Value::as_i64).unwrap_or(587) as u16;
    let user = cfg_str(c, "smtp_user").unwrap_or_default();
    let pass = cfg_str(c, "smtp_pass").unwrap_or_default();
    let mail_from = cfg_str(c, "mail_from").ok_or("mail_from not configured")?;
    let mail_to = cfg_str(c, "mail_to").ok_or("mail_to not configured")?;
    let body = Message::builder()
        .from(parse_mailbox(&mail_from, "mail_from")?)
        .to(parse_mailbox(&mail_to, "mail_to")?)
        .subject(format!("IoT 告警：{}", msg.rule_name))
        .body(alert_text(msg))
        .map_err(|e| format!("build email: {e}"))?;
    let tls = match cfg_str(c, "smtp_tls").unwrap_or_else(|| "starttls".into()).as_str() {
        "implicit" => Tls::Wrapper(TlsParameters::new(host.clone()).map_err(|e| format!("tls params: {e}"))?),
        "none" => Tls::None,
        _ => Tls::Required(TlsParameters::new(host.clone()).map_err(|e| format!("tls params: {e}"))?),
    };
    let mailer = SmtpTransport::builder_dangerous(&host)
        .port(port)
        .hello_name(ClientId::Domain("localhost".to_string()))
        .tls(tls)
        .credentials(Credentials::new(user, pass))
        .build();
    mailer.send(&body).map_err(|e| format!("smtp send: {e}"))?;
    Ok(())
}

/// 钉钉/企微 webhook：同为 text 消息载荷，仅 URL 不同。
async fn send_webhook(
    http: &reqwest::Client,
    ch: &NotifyChannel,
    msg: &AlertMessage,
) -> Result<(), String> {
    let url = cfg_str(&ch.config, "webhook_url").ok_or("webhook_url not configured")?;
    let payload = serde_json::json!({
        "msgtype": "text",
        "text": { "content": alert_text(msg) },
    });
    let resp = http
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("webhook request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("webhook non-2xx: {}", resp.status()));
    }
    Ok(())
}

/// 单渠道发送入口：渠道名白名单在 store::validate_channel 已保证。
pub async fn send_channel(
    ch: &NotifyChannel,
    msg: &AlertMessage,
    http: &reqwest::Client,
) -> Result<(), String> {
    match ch.channel.as_str() {
        CHANNEL_EMAIL => {
            tokio::task::spawn_blocking({
                let ch = ch.clone();
                let msg = msg.clone();
                move || send_email(&ch, &msg)
            })
            .await
            .map_err(|e| format!("email task join: {e}"))?
        }
        CHANNEL_DINGTALK | CHANNEL_WECOM => send_webhook(http, ch, msg).await,
        other => Err(format!("unknown channel: {other}")),
    }
}

/// 触发路径：对每个启用的渠道 spawn 独立任务发送，失败仅记日志，不阻塞告警主流程。
pub async fn dispatch(
    channels: Vec<NotifyChannel>,
    msg: AlertMessage,
    http: reqwest::Client,
) {
    for ch in channels.into_iter().filter(|c| c.enabled) {
        let http = http.clone();
        let msg = msg.clone();
        tokio::spawn(async move {
            if let Err(e) = send_channel(&ch, &msg, &http).await {
                tracing::warn!(channel = %ch.channel, error = %e, "notify channel delivery failed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg() -> AlertMessage {
        AlertMessage {
            rule_id: "r1".into(),
            rule_name: "高温告警".into(),
            tenant_id: "t1".into(),
            device_id: "d1".into(),
            code: "temp".into(),
            operator: "gt".into(),
            threshold: 30.0,
            value: json!(42),
            ts: 1700000000,
        }
    }

    fn channel(channel: &str, config: Value) -> NotifyChannel {
        NotifyChannel {
            id: "c1".into(),
            tenant_id: "t1".into(),
            channel: channel.into(),
            config,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn alert_text_contains_key_fields() {
        let t = alert_text(&msg());
        assert!(t.contains("高温告警"));
        assert!(t.contains("temp gt 30"));
        assert!(t.contains("42"));
    }

    #[test]
    fn unknown_channel_rejected() {
        let ch = channel("sms", json!({}));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let http = reqwest::Client::new();
        let err = rt
            .block_on(send_channel(&ch, &msg(), &http))
            .unwrap_err();
        assert!(err.contains("unknown channel"));
    }

    #[tokio::test]
    async fn webhook_channel_reaches_mock_server() {
        // 本地 mock webhook 目标：收到 POST 即记录载荷
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let store = received.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/hook",
                axum::routing::post(move |body: String| {
                    let store = store.clone();
                    async move {
                        store.lock().unwrap().push(body);
                        axum::http::StatusCode::OK
                    }
                }),
            );
            axum::serve(listener, app).await.unwrap();
        });

        let ch = channel(
            CHANNEL_WECOM,
            json!({ "webhook_url": format!("http://{addr}/hook") }),
        );
        let http = reqwest::Client::new();
        send_channel(&ch, &msg(), &http).await.unwrap();
        let got = received.lock().unwrap();
        assert_eq!(got.len(), 1);
        let parsed: Value = serde_json::from_str(&got[0]).unwrap();
        assert_eq!(parsed["msgtype"], "text");
        assert!(parsed["text"]["content"].as_str().unwrap().contains("高温告警"));
    }
}
