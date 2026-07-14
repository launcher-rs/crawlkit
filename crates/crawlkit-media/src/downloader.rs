//! 异步媒体下载器
//!
//! 支持：
//! - 并发下载（信号量限流）
//! - 指数退避重试
//! - SHA256 哈希
//! - Base64 编码
//! - 文件保存

#![cfg_attr(not(feature = "downloader"), allow(unused))]

use futures::stream::{self, StreamExt};
use sha2::{Sha256, Digest};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use url::Url;

use crate::types::{
    DownloadConfig, DownloadResult, MediaError, MediaResult, MediaType,
};

// ============================================================================
// MediaDownloader
// ============================================================================

/// 可配置的媒体下载器
#[derive(Debug, Clone)]
pub struct MediaDownloader {
    client: reqwest::Client,
    config: DownloadConfig,
    semaphore: Arc<Semaphore>,
}

impl Default for MediaDownloader {
    fn default() -> Self {
        Self::new(DownloadConfig::default())
    }
}

impl MediaDownloader {
    /// 使用指定配置创建下载器
    ///
    /// 自动读取 `PROXY_URL`/`PROXY_USER`/`PROXY_PASS` 环境变量配置代理。
    pub fn new(config: DownloadConfig) -> Self {
        let mut builder = reqwest::Client::builder();
        builder = builder
            .timeout(Duration::from_secs(config.timeout_secs))
            .user_agent(&config.user_agent);

        // 读取代理环境变量（与 ReqwestClient 保持一致）
        if let Ok(proxy_url) = std::env::var("PROXY_URL") {
            let proxy_user = std::env::var("PROXY_USER").unwrap_or_default();
            let proxy_pass = std::env::var("PROXY_PASS").unwrap_or_default();
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy.basic_auth(&proxy_user, &proxy_pass));
            }
        }

        let client = builder.build().unwrap_or_default();

        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));

        Self { client, config, semaphore }
    }

    /// 使用自定义 HTTP 客户端创建下载器
    pub fn with_client(client: reqwest::Client, config: DownloadConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
        Self { client, config, semaphore }
    }

    /// 下载单个 URL
    pub async fn download(&self, url: &str) -> MediaResult<DownloadResult> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| MediaError::Download(e.to_string()))?;

        self.download_with_retry(url).await
    }

    /// 带重试的下载
    async fn download_with_retry(&self, url: &str) -> MediaResult<DownloadResult> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(self.config.retry_delay_ms * (1 << (attempt - 1)));
                tokio::time::sleep(delay).await;
            }

            match self.do_download(url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("下载失败 (尝试 {}/{}): {}", attempt + 1, self.config.max_retries + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| MediaError::Download("未知错误".to_string())))
    }

    /// 执行实际下载
    async fn do_download(&self, url: &str) -> MediaResult<DownloadResult> {
        let response = self.client.get(url)
            .send()
            .await
            .map_err(|e| MediaError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MediaError::Http(
                response.status().as_u16(),
                response.status().to_string(),
            ));
        }

        let content_type = response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).to_string());

        let content_length = response.headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());

        if let Some(max_size) = self.config.max_file_size
            && let Some(size) = content_length
                && size > max_size {
                    return Err(MediaError::FileTooLarge(size, max_size));
                }

        let bytes = response.bytes()
            .await
            .map_err(|e| MediaError::Download(e.to_string()))?;

        let actual_size = bytes.len() as u64;
        if let Some(max_size) = self.config.max_file_size
            && actual_size > max_size {
                return Err(MediaError::FileTooLarge(actual_size, max_size));
            }

        let hash = compute_sha256(&bytes);
        let media_type = detect_media_type(&content_type, url);

        let base64 = if self.config.encode_base64 {
            use base64::Engine;
            Some(base64::engine::general_purpose::STANDARD.encode(&bytes))
        } else {
            None
        };

        Ok(DownloadResult {
            url: url.to_string(),
            bytes: bytes.to_vec(),
            content_type,
            size: content_length.unwrap_or(actual_size),
            hash,
            media_type,
            base64,
        })
    }

    /// 并发下载多个 URL
    pub async fn download_many(&self, urls: &[String]) -> Vec<MediaResult<DownloadResult>> {
        stream::iter(urls)
            .map(|url| {
                let downloader = self.clone();
                async move {
                    downloader.download(url).await
                }
            })
            .buffer_unordered(self.config.max_concurrent)
            .collect()
            .await
    }

    /// 下载并保存到文件
    pub async fn download_to_file(&self, url: &str, path: &Path) -> MediaResult<DownloadResult> {
        let result = self.download(url).await?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| MediaError::Io(e.to_string()))?;
        }

        tokio::fs::write(path, &result.bytes)
            .await
            .map_err(|e| MediaError::Io(e.to_string()))?;

        Ok(result)
    }

    /// 并发下载并保存到目录
    pub async fn download_many_to_dir(
        &self,
        urls: &[String],
        dir: &Path,
    ) -> Vec<MediaResult<(String, std::path::PathBuf)>> {
        stream::iter(urls)
            .map(|url| {
                let downloader = self.clone();
                let dir = dir.to_path_buf();
                async move {
                    let filename = url_to_filename(url);
                    let path = dir.join(&filename);

                    downloader.download_to_file(url, &path)
                        .await
                        .map(|_| (url.clone(), path))
                }
            })
            .buffer_unordered(self.config.max_concurrent)
            .collect()
            .await
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 计算 SHA256 哈希
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// 检测媒体类型（优先使用 Content-Type，其次 URL 扩展名）
pub fn detect_media_type(content_type: &Option<String>, url: &str) -> MediaType {
    if let Some(ct) = content_type {
        if ct.starts_with("image/") { return MediaType::Image; }
        if ct.starts_with("video/") { return MediaType::Video; }
        if ct.starts_with("audio/") { return MediaType::Audio; }
        if ct.contains("pdf") { return MediaType::Document; }
        if ct.contains("document") || ct.contains("spreadsheet") || ct.contains("presentation") {
            return MediaType::Document;
        }
    }

    let u = url.to_lowercase();

    if u.ends_with(".jpg") || u.ends_with(".jpeg") ||
       u.ends_with(".png") || u.ends_with(".gif") ||
       u.ends_with(".webp") || u.ends_with(".svg") ||
       u.ends_with(".avif") {
        return MediaType::Image;
    }

    if u.ends_with(".mp4") || u.ends_with(".webm") ||
       u.ends_with(".avi") || u.ends_with(".mov") ||
       u.ends_with(".mkv") {
        return MediaType::Video;
    }

    if u.ends_with(".mp3") || u.ends_with(".wav") ||
       u.ends_with(".ogg") || u.ends_with(".flac") ||
       u.ends_with(".aac") {
        return MediaType::Audio;
    }

    if u.ends_with(".pdf") || u.ends_with(".doc") ||
       u.ends_with(".docx") || u.ends_with(".xls") ||
       u.ends_with(".xlsx") || u.ends_with(".ppt") ||
       u.ends_with(".pptx") {
        return MediaType::Document;
    }

    MediaType::Other
}

/// 从 URL 生成文件名
pub fn url_to_filename(url: &str) -> String {
    if let Ok(parsed) = Url::parse(url) {
        let path = parsed.path();
        let filename = path.rsplit('/').next().unwrap_or("download");

        if filename.is_empty() || filename == "/" {
            let hash = &compute_sha256(url.as_bytes())[..12];
            return format!("download_{hash}");
        }

        sanitize_filename(filename)
    } else {
        let hash = &compute_sha256(url.as_bytes())[..12];
        format!("download_{hash}")
    }
}

/// 清理文件名（去除非文件系统安全字符）
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 判断 URL 是否可下载
pub fn is_downloadable(url: &str) -> bool {
    let u = url.to_lowercase();

    if u.starts_with("data:") || u.starts_with("javascript:") {
        return false;
    }

    u.starts_with("http://") || u.starts_with("https://")
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速下载到字节
pub async fn download_bytes(url: &str) -> MediaResult<Vec<u8>> {
    let downloader = MediaDownloader::default();
    let result = downloader.download(url).await?;
    Ok(result.bytes)
}

/// 快速下载并获取哈希
pub async fn download_with_hash(url: &str) -> MediaResult<(Vec<u8>, String)> {
    let downloader = MediaDownloader::default();
    let result = downloader.download(url).await?;
    Ok((result.bytes, result.hash))
}

/// 快速下载为 Base64
pub async fn download_to_base64(url: &str) -> MediaResult<String> {
    let config = DownloadConfig {
        encode_base64: true,
        ..Default::default()
    };
    let downloader = MediaDownloader::new(config);
    let result = downloader.download(url).await?;
    result.base64.ok_or_else(|| MediaError::Download("Base64 编码失败".to_string()))
}

/// 下载并保存到文件
pub async fn save_to_file(url: &str, path: &Path) -> MediaResult<()> {
    let downloader = MediaDownloader::default();
    downloader.download_to_file(url, path).await?;
    Ok(())
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sha256() {
        let data = b"Hello, World!";
        let hash = compute_sha256(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_detect_media_type_from_content_type() {
        assert_eq!(detect_media_type(&Some("image/png".to_string()), ""), MediaType::Image);
        assert_eq!(detect_media_type(&Some("video/mp4".to_string()), ""), MediaType::Video);
        assert_eq!(detect_media_type(&Some("audio/mpeg".to_string()), ""), MediaType::Audio);
        assert_eq!(detect_media_type(&Some("application/pdf".to_string()), ""), MediaType::Document);
    }

    #[test]
    fn test_detect_media_type_from_url() {
        assert_eq!(detect_media_type(&None, "https://example.com/image.png"), MediaType::Image);
        assert_eq!(detect_media_type(&None, "https://example.com/video.mp4"), MediaType::Video);
        assert_eq!(detect_media_type(&None, "https://example.com/audio.mp3"), MediaType::Audio);
        assert_eq!(detect_media_type(&None, "https://example.com/doc.pdf"), MediaType::Document);
        assert_eq!(detect_media_type(&None, "https://example.com/unknown"), MediaType::Other);
    }

    #[test]
    fn test_url_to_filename() {
        assert_eq!(url_to_filename("https://example.com/images/photo.jpg"), "photo.jpg");
        assert!(url_to_filename("https://example.com/").starts_with("download_"));
    }

    #[test]
    fn test_is_downloadable() {
        assert!(is_downloadable("https://example.com/file.jpg"));
        assert!(is_downloadable("http://example.com/file.pdf"));
        assert!(!is_downloadable("data:image/png;base64,abc"));
        assert!(!is_downloadable("javascript:void(0)"));
        assert!(!is_downloadable("/relative/path"));
    }

    #[test]
    fn test_download_config_default() {
        let config = DownloadConfig::default();
        assert!(config.max_concurrent > 0);
        assert!(config.timeout_secs > 0);
    }

    #[test]
    fn test_downloader_creation() {
        let downloader = MediaDownloader::default();
        assert!(downloader.config.max_concurrent > 0);
    }
}
