//! HTTP 客户端抽象 trait
//!
//! `HttpClient` trait 定义了所有 HTTP 后端必须实现的统一接口。
//! 具体的实现（如 ReqwestClient、WreqClient、ChromeClient）位于 `crawlkit-fetcher` crate。

use async_trait::async_trait;
use std::collections::HashMap;

use crate::error::Result;
use crate::response::Response;

/// HTTP 客户端抽象 trait
///
/// 实现此 trait 即可接入框架，例如：
/// - `ReqwestClient`（默认，基于 reqwest）
/// - `WreqClient`（基于 wreq）
/// - Chrome CDP 客户端
/// - Mock 客户端（用于测试）
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// 发送 GET 请求
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response>;

    /// 发送 POST 请求
    async fn post(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<Response>;

    /// 返回客户端名称（用于日志/调试）
    fn name(&self) -> &str;
}
