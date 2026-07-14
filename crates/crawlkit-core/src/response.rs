//! HTTP 响应封装

use std::collections::HashMap;

/// 统一的 HTTP 响应结构
///
/// 无论底层使用 reqwest、wreq 还是 Chrome，都统一转换为该类型。
#[derive(Debug, Clone)]
pub struct Response {
    /// 最终请求的 URL（可能经过重定向）
    pub url: String,
    /// HTTP 状态码
    pub status: u16,
    /// 响应头
    pub headers: HashMap<String, String>,
    /// 响应体（文本）
    pub body: String,
}

impl Response {
    /// 状态码是否为 2xx
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// 获取 Content-Type
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("content-type").map(String::as_str)
    }

    /// 是否为 HTML 内容
    ///
    /// 匹配 `text/html`、`application/xhtml+xml`，或 body 以 `<!doctype html` / `<html` 开头。
    pub fn is_html(&self) -> bool {
        if let Some(ct) = self.content_type() {
            let ct_lower = ct.to_ascii_lowercase();
            if ct_lower.contains("text/html") || ct_lower.contains("application/xhtml+xml") {
                return true;
            }
        }
        let trimmed = self.body.trim_start();
        let lower = trimmed[..trimmed.len().min(64)].to_ascii_lowercase();
        lower.starts_with("<!doctype html") || lower.starts_with("<html")
    }

    /// 检测响应是否为机器人验证页面
    ///
    /// 很多反爬服务（PerimeterX、Cloudflare、DataDome、Akamai 等）在检测到爬虫时
    /// 返回 HTTP 200，但 body 内容是 CAPTCHA/challenge 页面，不是真实内容。
    /// 此方法通过检测 body 中的特征关键词来识别这类伪成功响应。
    pub fn is_bot_challenge(&self) -> bool {
        if !self.is_html() {
            return false;
        }
        let body_lower = self.body.to_ascii_lowercase();

        // PerimeterX / HUMAN Security
        if body_lower.contains("_pxhd")
            || body_lower.contains("_pxuuid")
            || body_lower.contains("px-captcha")
            || body_lower.contains("pxcaptcha")
            || body_lower.contains("_pxappappid")
            || body_lower.contains("px-cloud.net")
            || body_lower.contains("humansecurity.com")
        {
            return true;
        }

        // Cloudflare challenge
        if body_lower.contains("cf-challenge")
            || body_lower.contains("challenge-platform")
            || body_lower.contains("ray id")
                && body_lower.contains("cloudflare")
                && body_lower.contains("enable javascript")
        {
            return true;
        }

        // DataDome
        if body_lower.contains("datadome")
            || body_lower.contains("captcha-delivery")
        {
            return true;
        }

        // Akamai Bot Manager
        if body_lower.contains("akamai")
            && (body_lower.contains("bot") || body_lower.contains("challenge"))
        {
            return true;
        }

        // 通用 challenge 关键词组合（需同时命中多个以减少误判）
        if body_lower.contains("access to this page")
            && body_lower.contains("denied")
            && (body_lower.contains("captcha")
                || body_lower.contains("human")
                || body_lower.contains("press & hold")
                || body_lower.contains("verify you are"))
        {
            return true;
        }

        // 常见验证页面 title
        if (body_lower.contains("<title>")
            && (body_lower.contains("attention required")
                || body_lower.contains("access denied")
                || body_lower.contains("please verify")
                || body_lower.contains("security check")
                || body_lower.contains("robot")
                || body_lower.contains("blocked")))
            && body_lower.contains("challenge")
        {
            return true;
        }

        false
    }
}
