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
}
