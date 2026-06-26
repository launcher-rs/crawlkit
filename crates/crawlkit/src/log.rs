//! 日志初始化模块
//!
//! 提供便捷函数，帮助用户快速启用 tracing 日志输出。
//!
//! # 快速上手
//! ```rust,no_run
//! // 初始化 info 级别日志
//! crawlkit::log::init();
//!
//! // 使用 RUST_LOG 环境变量自定义过滤
//! crawlkit::log::init_with_env();
//!
//! // 启用 DEBUG 级别（含请求/响应回调详情）
//! crawlkit::log::init_debug();
//! ```

use tracing_subscriber::fmt;
use tracing_subscriber::EnvFilter;

/// 初始化 tracing 日志（默认 info 级别）
///
/// 使用 `RUST_LOG` 环境变量可覆盖默认级别。
/// 若未设置 `RUST_LOG`，则使用 `crawlkit=info` 作为默认过滤规则。
///
/// # 示例
/// ```rust,no_run
/// crawlkit::log::init();
/// ```
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("crawlkit=info"));

    fmt().with_env_filter(filter).init();
}

/// 使用 `RUST_LOG` 环境变量初始化日志
///
/// 若未设置 `RUST_LOG`，则不应用任何过滤（显示所有 crate 的 info 日志）。
///
/// # 示例
/// ```bash
/// RUST_LOG=debug cargo run --example callback
/// ```
pub fn init_with_env() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt().with_env_filter(filter).init();
}

/// 初始化 DEBUG 级别日志
///
/// 显示所有 crawlkit 相关的 debug 日志，包括请求/响应回调详情。
///
/// # 示例
/// ```rust,no_run
/// crawlkit::log::init_debug();
/// ```
pub fn init_debug() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("crawlkit=debug"));

    fmt().with_env_filter(filter).init();
}

/// 初始化带时间戳的日志（INFO 级别）
///
/// 输出格式包含时间戳、级别和目标模块。
///
/// # 示例
/// ```rust,no_run
/// crawlkit::log::init_with_timestamp();
/// ```
pub fn init_with_timestamp() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("crawlkit=info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .init();
}
