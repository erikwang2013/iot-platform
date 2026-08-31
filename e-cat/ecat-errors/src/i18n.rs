//! 错误消息 i18n（C-线，最小落地）：默认英文，`Accept-Language` 以 zh 开头时
//! 查映射表返回中文。映射表只覆盖公共 API 常见错误（401/403/404/422/429、
//! 限流、登录等），未命中保持英文原文——含动态内容的错误（ID 内嵌等）
//! 不做逐条翻译。内部日志不经过这里，保持英文原文。

/// 从 Accept-Language 头提取语言：zh 前缀 → "zh"，其余 → None（英文）。
/// 只取第一个语言标签，忽略 q 值。
pub fn locale_from_accept_language(value: Option<&str>) -> Option<&'static str> {
    value
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|lang| lang.starts_with("zh"))
        .map(|_| "zh")
}

/// 按语言本地化错误消息：zh 命中映射表返回中文，否则返回英文原文。
pub fn localize(locale: Option<&str>, message: &str) -> String {
    if locale == Some("zh") && let Some(zh) = zh(message) {
        return zh.to_string();
    }
    message.to_string()
}

fn zh(message: &str) -> Option<&'static str> {
    Some(match message {
        // 认证/鉴权（401/403）
        "missing authorization token" => "缺少访问令牌",
        "invalid token" => "无效的访问令牌",
        "forbidden: insufficient role" => "权限不足，无法执行该操作",
        "admin role required" => "需要管理员权限",
        "invalid username or password" => "用户名或密码错误",
        "username and password required" => "用户名和密码不能为空",
        "invalid app_id or app_secret" => "无效的 app_id 或 app_secret",
        "app_id and app_secret required" => "app_id 和 app_secret 不能为空",
        // 限流（429）
        "rate limit exceeded" => "请求过于频繁，请稍后重试",
        // 404 常见业务错误
        "device not linked" => "设备未接入",
        "device not found" => "设备不存在",
        "rule not found" => "规则不存在",
        "alert not found" => "告警记录不存在",
        "channel not found" => "通知渠道不存在",
        "tenant not found" => "租户不存在",
        "user not found" => "用户不存在",
        "api key not found or already revoked" => "密钥不存在或已吊销",
        // 校验（400/422）常见错误
        "name required" => "名称不能为空",
        "name must be 1..128 chars" => "名称长度需在 1..128 字符之间",
        "status must be active|acknowledged" => "status 必须为 active 或 acknowledged",
        "scope must be read|write|command" => "scope 必须为 read、write 或 command",
        // 服务端兜底
        "internal error" => "服务内部错误",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_detects_zh_prefix() {
        assert_eq!(locale_from_accept_language(Some("zh-CN,zh;q=0.9")), Some("zh"));
        assert_eq!(locale_from_accept_language(Some("zh")), Some("zh"));
        assert_eq!(locale_from_accept_language(Some("en-US,en;q=0.9")), None);
        assert_eq!(locale_from_accept_language(Some("fr")), None);
        assert_eq!(locale_from_accept_language(None), None);
        // 只认第一个标签：zh 必须在首位才生效
        assert_eq!(locale_from_accept_language(Some("en,zh")), None);
    }

    #[test]
    fn localize_zh_hits_map_else_english() {
        assert_eq!(localize(Some("zh"), "rate limit exceeded"), "请求过于频繁，请稍后重试");
        assert_eq!(localize(Some("zh"), "invalid token"), "无效的访问令牌");
        // 未命中保持英文
        assert_eq!(localize(Some("zh"), "device abc-1 not found"), "device abc-1 not found");
        // 非 zh 一律英文
        assert_eq!(localize(None, "invalid token"), "invalid token");
        assert_eq!(localize(Some("en"), "invalid token"), "invalid token");
    }

    #[test]
    fn error_message_localized() {
        let err = crate::Error::new(crate::ErrorCode::NotFound, "device_not_linked", "device not linked");
        assert_eq!(err.message_localized(Some("zh")), "设备未接入");
        assert_eq!(err.message_localized(None), "device not linked");
    }
}
