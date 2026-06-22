//! 请求与响应类型

use std::collections::HashMap;

/// 爬虫请求封装
///
/// 每次访问都会构造一个 Request，经过回调链后交给 HttpClient 执行
#[derive(Debug, Clone)]
pub struct Request {
    /// 目标 URL
    pub url: String,
    /// HTTP 方法（目前主要用 GET）
    pub method: String,
    /// 自定义请求头
    pub headers: HashMap<String, String>,
    /// 是否允许跟踪此请求的链接（类似 colly 的 AllowURLRevisit）
    pub allow_revisit: bool,
    /// 用户自定义上下文，可在回调间传递数据
    pub context: HashMap<String, String>,
}

impl Request {
    /// 创建新的 GET 请求
    pub fn get(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "GET".into(),
            headers: HashMap::new(),
            allow_revisit: false,
            context: HashMap::new(),
        }
    }

    /// 设置自定义请求头
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// 往上下文中写入键值对（回调间传递数据用）
    pub fn set_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }
}
