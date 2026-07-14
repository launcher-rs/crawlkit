//! 框架统一的错误类型定义

use thiserror::Error;

/// 框架统一错误类型
///
/// 覆盖 HTTP 请求、URL 解析、HTML 解析、回调执行等场景。
#[derive(Error, Debug)]
pub enum CrawlError {
    /// HTTP 请求失败
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
    #[error("所有请求器均失败: {0}")]
    AllFetchersFailed(String),

    /// 检测到机器人验证页面（HTTP 200 但内容为 CAPTCHA/challenge）
    #[error("机器人验证页面: {0}")]
    BotChallenge(String),

    /// 锁中毒
    #[error("锁错误: {0}")]
    Lock(String),
}

/// 框架统一的 Result 别名
pub type Result<T> = std::result::Result<T, CrawlError>;
