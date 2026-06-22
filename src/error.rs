//! 错误类型定义

use thiserror::Error;

/// 框架统一错误类型
#[derive(Error, Debug)]
pub enum CollyError {
    /// HTTP 请求错误
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    /// URL 解析错误
    #[error("URL 解析失败: {0}")]
    Url(#[from] url::ParseError),

    /// HTML 解析错误
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
}

pub type Result<T> = std::result::Result<T, CollyError>;

impl From<anyhow::Error> for CollyError {
    fn from(err: anyhow::Error) -> Self {
        CollyError::Callback(err.to_string())
    }
}
