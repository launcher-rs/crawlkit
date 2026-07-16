//! 框架统一的错误类型定义
//!
//! 所有错误通过 [`CrawlError`] 枚举统一表示，便于上层按类型分发处理。

use thiserror::Error;

/// 框架统一错误类型
///
/// 覆盖 HTTP 请求、URL 解析、HTML 解析、回调执行、拦截检测等场景。
///
/// # 拦截相关错误
///
/// - [`BotChallenge`](CrawlError::BotChallenge) — HTTP 200 但内容为机器人验证页面（CAPTCHA/challenge）
/// - [`AccessDenied`](CrawlError::AccessDenied) — HTTP 403/401 等访问被拒绝（CDN/WAF 拦截）
///
/// 在 [`CompositeFetcher`](crawlkit_fetcher::CompositeFetcher) 中，这两个错误会触发自动故障转移；
/// 在 [`Collector`](crawlkit::Collector) 中，会触发 `on_error` 回调并返回 `Err`。
#[derive(Error, Debug)]
pub enum CrawlError {
    /// HTTP 请求失败（网络错误、超时等）
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    /// URL 解析失败
    #[error("URL 解析失败: {0}")]
    Url(#[from] url::ParseError),

    /// HTML 解析失败
    #[error("HTML 解析失败: {0}")]
    Html(String),

    /// 回调执行错误
    #[error("回调执行出错: {0}")]
    Callback(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// CSS 选择器解析错误
    #[error("CSS 选择器解析失败 '{selector}': {message}")]
    Selector { selector: String, message: String },

    /// 可读性提取错误
    #[error("可读性提取失败: {0}")]
    Readability(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    Config(String),

    /// 所有请求器均失败
    ///
    /// 在 `CompositeFetcher` 中，当所有后端都返回错误（网络错误、BotChallenge、AccessDenied）时触发。
    #[error("所有请求器均失败: {0}")]
    AllFetchersFailed(String),

    /// 检测到机器人验证页面
    ///
    /// HTTP 200 但 body 内容为 CAPTCHA/challenge 页面（如 PerimeterX、Cloudflare、DataDome）。
    ///
    /// 由 [`Response::is_bot_challenge()`](crawlkit_core::response::Response::is_bot_challenge)
    /// 内置检测，或通过自定义 `on_detect_bot_challenge` 回调触发。
    #[error("机器人验证页面: {0}")]
    BotChallenge(String),

    /// 访问被拒绝
    ///
    /// HTTP 403/401 等拒绝访问响应，body 为 CDN/WAF 拦截页面（如 Akamai、Cloudflare WAF）。
    ///
    /// 由 [`Response::is_access_denied()`](crawlkit_core::response::Response::is_access_denied)
    /// 内置检测，或通过自定义 `on_detect_access_denied` 回调触发。
    #[error("访问被拒绝: {0}")]
    AccessDenied(String),

    /// 锁中毒
    #[error("锁错误: {0}")]
    Lock(String),
}

/// 框架统一 Result 别名
pub type Result<T> = std::result::Result<T, CrawlError>;
