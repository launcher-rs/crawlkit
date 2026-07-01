//! 爬虫请求封装

use std::collections::HashMap;

/// 爬虫请求封装
///
/// 每次访问都会构造一个 Request，经过回调链后交给 HttpClient 执行。
#[derive(Debug, Clone)]
pub struct Request {
    /// 目标 URL
    pub url: String,
    /// HTTP 方法
    pub method: String,
    /// 自定义请求头
    pub headers: HashMap<String, String>,
    /// 是否允许重复访问（跳过 visited 去重检查）
    pub allow_revisit: bool,
    /// 用户自定义上下文，可在回调间传递数据
    pub context: HashMap<String, String>,
    /// POST 请求体
    pub body: Vec<u8>,
    /// 是否已被回调中止（调用 `abort()` 后为 true）
    pub aborted: bool,
}

impl Request {
    /// 创建 GET 请求
    pub fn get(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "GET".into(),
            headers: HashMap::new(),
            allow_revisit: false,
            context: HashMap::new(),
            body: Vec::new(),
            aborted: false,
        }
    }

    /// 创建 POST 请求
    pub fn post(url: &str, body: Vec<u8>) -> Self {
        Self {
            url: url.to_string(),
            method: "POST".into(),
            headers: HashMap::new(),
            allow_revisit: false,
            context: HashMap::new(),
            body,
            aborted: false,
        }
    }

    /// 设置请求头
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// 写入上下文键值对（回调间传递数据用）
    pub fn set_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }

    /// 中止当前请求处理
    ///
    /// 在 `on_request` 回调中调用，后续的 HTTP 请求和所有回调（on_response / on_html / on_scraped）都不会执行。
    pub fn abort(&mut self) {
        self.aborted = true;
    }
}
