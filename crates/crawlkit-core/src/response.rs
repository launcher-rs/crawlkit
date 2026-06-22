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
        self.headers.get("content-type").map(|s| s.as_str())
    }

    /// 是否为 HTML 内容
    pub fn is_html(&self) -> bool {
        self.content_type()
            .map(|ct| ct.contains("text/html"))
            .unwrap_or(false)
    }
}
