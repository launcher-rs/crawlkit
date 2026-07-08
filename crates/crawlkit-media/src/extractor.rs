//! 媒体提取器统一 API
//!
//! 提供跨所有媒体类型的统一提取入口：
//! - 图片（srcset、懒加载）
//! - 视频（HTML5 + 嵌入平台）
//! - 音频（HTML5 + 流媒体平台）
//! - 文档（PDF、Office）
//! - 嵌入内容（地图、社交、小部件）
//!
//! 支持 builder 模式配置

use scraper::Html;
use url::Url;

use crate::types::{
    ExtractedMedia, MediaConfig, MediaResult, MediaType,
    ImageMedia, VideoMedia, AudioMedia, DocumentMedia, EmbeddedMedia,
};
use crate::{images, videos, audio, documents, embedded};

// ============================================================================
// MediaExtractor
// ============================================================================

/// 可配置的媒体提取器
#[derive(Debug, Clone, Default)]
pub struct MediaExtractor {
    config: MediaConfig,
    base_url: Option<Url>,
}

impl MediaExtractor {
    /// 创建默认配置的提取器
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用指定配置创建提取器
    pub fn with_config(config: MediaConfig) -> Self {
        Self {
            config,
            base_url: None,
        }
    }

    /// 设置基础 URL（用于解析相对路径）
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = Url::parse(url).ok();
        self
    }

    /// 从 Url 对象设置基础 URL
    pub fn with_base(mut self, url: Url) -> Self {
        self.base_url = Some(url);
        self
    }

    /// 获取当前配置
    pub fn config(&self) -> &MediaConfig {
        &self.config
    }

    /// 获取基础 URL
    pub fn base_url(&self) -> Option<&Url> {
        self.base_url.as_ref()
    }

    // ========================================================================
    // 提取方法
    // ========================================================================

    /// 从 HTML 字符串提取所有媒体
    pub fn extract_all(&self, html: &str) -> MediaResult<ExtractedMedia> {
        let document = Html::parse_document(html);
        self.extract_from_document(&document)
    }

    /// 从已解析的 HTML 文档提取所有媒体
    pub fn extract_from_document(&self, document: &Html) -> MediaResult<ExtractedMedia> {
        let mut result = ExtractedMedia::default();

        if self.config.extract_images {
            let mut images = images::extract_images(document, self.base_url.as_ref());

            if self.config.filter_placeholders {
                images = images::filter_placeholders(images);
            }

            if let Some(min_w) = self.config.min_image_width {
                images.retain(|img| img.width.unwrap_or(u32::MAX) >= min_w);
            }
            if let Some(min_h) = self.config.min_image_height {
                images.retain(|img| img.height.unwrap_or(u32::MAX) >= min_h);
            }

            result.images = images;
        }

        if self.config.extract_videos {
            result.videos = videos::extract_videos(document, self.base_url.as_ref());
        }

        if self.config.extract_audio {
            result.audio = audio::extract_audio(document, self.base_url.as_ref());
        }

        if self.config.extract_documents {
            result.documents = documents::extract_documents(document, self.base_url.as_ref());
        }

        if self.config.extract_embeds {
            result.embeds = embedded::extract_embeds(document, self.base_url.as_ref());
        }

        Ok(result)
    }

    // ========================================================================
    // 单类型提取
    // ========================================================================

    /// 仅提取图片
    pub fn extract_images(&self, html: &str) -> MediaResult<Vec<ImageMedia>> {
        let document = Html::parse_document(html);
        Ok(images::extract_images(&document, self.base_url.as_ref()))
    }

    /// 仅提取视频
    pub fn extract_videos(&self, html: &str) -> MediaResult<Vec<VideoMedia>> {
        let document = Html::parse_document(html);
        Ok(videos::extract_videos(&document, self.base_url.as_ref()))
    }

    /// 仅提取音频
    pub fn extract_audio(&self, html: &str) -> MediaResult<Vec<AudioMedia>> {
        let document = Html::parse_document(html);
        Ok(audio::extract_audio(&document, self.base_url.as_ref()))
    }

    /// 仅提取文档
    pub fn extract_documents(&self, html: &str) -> MediaResult<Vec<DocumentMedia>> {
        let document = Html::parse_document(html);
        Ok(documents::extract_documents(&document, self.base_url.as_ref()))
    }

    /// 仅提取嵌入内容
    pub fn extract_embeds(&self, html: &str) -> MediaResult<Vec<EmbeddedMedia>> {
        let document = Html::parse_document(html);
        Ok(embedded::extract_embeds(&document, self.base_url.as_ref()))
    }

    // ========================================================================
    // URL 收集
    // ========================================================================

    /// 获取所有媒体 URL
    pub fn get_all_urls(&self, html: &str) -> Vec<String> {
        let extracted = self.extract_all(html).unwrap_or_default();
        extracted.all_urls()
    }

    /// 按类型获取媒体 URL
    pub fn get_urls_by_type(&self, html: &str, media_type: MediaType) -> Vec<String> {
        let base = self.base_url.as_ref().map(|u| u.as_str());
        match media_type {
            MediaType::Image => images::get_image_urls(html, base),
            MediaType::Video => videos::get_video_urls(html, base),
            MediaType::Audio => audio::get_audio_urls(html, base),
            MediaType::Document => documents::get_document_urls(html, base),
            MediaType::Embedded => embedded::get_embed_urls(html, base),
            MediaType::Other => Vec::new(),
        }
    }

    // ========================================================================
    // 存在性检查
    // ========================================================================

    /// 检查 HTML 是否包含任何媒体
    pub fn has_media(&self, html: &str) -> bool {
        let document = Html::parse_document(html);
        images::has_images(&document)
            || videos::has_videos(&document)
            || audio::has_audio(&document)
            || documents::has_documents(&document)
            || embedded::has_embeds(&document)
    }

    /// 检查 HTML 是否包含指定类型的媒体
    pub fn has_media_type(&self, html: &str, media_type: MediaType) -> bool {
        let document = Html::parse_document(html);
        match media_type {
            MediaType::Image => images::has_images(&document),
            MediaType::Video => videos::has_videos(&document),
            MediaType::Audio => audio::has_audio(&document),
            MediaType::Document => documents::has_documents(&document),
            MediaType::Embedded => embedded::has_embeds(&document),
            MediaType::Other => false,
        }
    }

    // ========================================================================
    // 统计
    // ========================================================================

    /// 获取各类型媒体数量
    pub fn count_media(&self, html: &str) -> MediaCounts {
        let extracted = self.extract_all(html).unwrap_or_default();
        MediaCounts {
            images: extracted.images.len(),
            videos: extracted.videos.len(),
            audio: extracted.audio.len(),
            documents: extracted.documents.len(),
            embeds: extracted.embeds.len(),
            total: extracted.total_count(),
        }
    }
}

// ============================================================================
// MediaCounts
// ============================================================================

/// 媒体数量统计
#[derive(Debug, Clone, Default)]
pub struct MediaCounts {
    pub images: usize,
    pub videos: usize,
    pub audio: usize,
    pub documents: usize,
    pub embeds: usize,
    pub total: usize,
}

impl MediaCounts {
    /// 是否包含任何媒体
    pub fn has_any(&self) -> bool {
        self.total > 0
    }

    /// 是否包含指定类型
    pub fn has_type(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Image => self.images > 0,
            MediaType::Video => self.videos > 0,
            MediaType::Audio => self.audio > 0,
            MediaType::Document => self.documents > 0,
            MediaType::Embedded => self.embeds > 0,
            MediaType::Other => false,
        }
    }
}

// ============================================================================
// MediaExtractorBuilder
// ============================================================================

/// MediaExtractor 构建器
#[derive(Debug, Clone, Default)]
pub struct MediaExtractorBuilder {
    config: MediaConfig,
    base_url: Option<String>,
}

impl MediaExtractorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extract_images(mut self, enabled: bool) -> Self {
        self.config.extract_images = enabled;
        self
    }

    pub fn extract_videos(mut self, enabled: bool) -> Self {
        self.config.extract_videos = enabled;
        self
    }

    pub fn extract_audio(mut self, enabled: bool) -> Self {
        self.config.extract_audio = enabled;
        self
    }

    pub fn extract_documents(mut self, enabled: bool) -> Self {
        self.config.extract_documents = enabled;
        self
    }

    pub fn extract_embeds(mut self, enabled: bool) -> Self {
        self.config.extract_embeds = enabled;
        self
    }

    pub fn filter_placeholders(mut self, enabled: bool) -> Self {
        self.config.filter_placeholders = enabled;
        self
    }

    pub fn include_data_urls(mut self, enabled: bool) -> Self {
        self.config.include_data_urls = enabled;
        self
    }

    pub fn min_image_size(mut self, width: u32, height: u32) -> Self {
        self.config.min_image_width = Some(width);
        self.config.min_image_height = Some(height);
        self
    }

    pub fn base_url(mut self, url: &str) -> Self {
        self.base_url = Some(url.to_string());
        self
    }

    pub fn build(self) -> MediaExtractor {
        let mut extractor = MediaExtractor::with_config(self.config);
        if let Some(url) = self.base_url {
            extractor = extractor.with_base_url(&url);
        }
        extractor
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速提取所有媒体
pub fn extract_media(html: &str, base_url: Option<&str>) -> MediaResult<ExtractedMedia> {
    let mut extractor = MediaExtractor::new();
    if let Some(url) = base_url {
        extractor = extractor.with_base_url(url);
    }
    extractor.extract_all(html)
}

/// 快速检查是否包含任何媒体
pub fn has_any_media(html: &str) -> bool {
    MediaExtractor::new().has_media(html)
}

/// 快速获取媒体数量
pub fn count_all_media(html: &str) -> MediaCounts {
    MediaExtractor::new().count_media(html)
}

/// 获取所有媒体 URL
pub fn get_all_media_urls(html: &str, base_url: Option<&str>) -> Vec<String> {
    let mut extractor = MediaExtractor::new();
    if let Some(url) = base_url {
        extractor = extractor.with_base_url(url);
    }
    extractor.get_all_urls(html)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_HTML: &str = r#"
        <html>
        <body>
            <img src="https://example.com/image.jpg" alt="Test">
            <video src="https://example.com/video.mp4"></video>
            <audio src="https://example.com/audio.mp3"></audio>
            <a href="https://example.com/doc.pdf">PDF</a>
            <iframe src="https://www.google.com/maps/embed"></iframe>
        </body>
        </html>
    "#;

    #[test]
    fn test_extract_all() {
        let extractor = MediaExtractor::new();
        let result = extractor.extract_all(TEST_HTML).unwrap();

        assert!(!result.images.is_empty());
        assert!(!result.videos.is_empty());
        assert!(!result.audio.is_empty());
        assert!(!result.documents.is_empty());
        assert!(!result.embeds.is_empty());
    }

    #[test]
    fn test_extract_with_base_url() {
        let html = r#"<img src="/images/test.jpg">"#;
        let extractor = MediaExtractor::new()
            .with_base_url("https://example.com");

        let images = extractor.extract_images(html).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].absolute_url, Some("https://example.com/images/test.jpg".to_string()));
    }

    #[test]
    fn test_has_media() {
        let extractor = MediaExtractor::new();

        assert!(extractor.has_media(TEST_HTML));
        assert!(!extractor.has_media("<div>No media</div>"));
    }

    #[test]
    fn test_builder() {
        let extractor = MediaExtractorBuilder::new()
            .extract_images(true)
            .extract_videos(false)
            .filter_placeholders(true)
            .base_url("https://example.com")
            .build();

        assert!(extractor.config().extract_images);
        assert!(!extractor.config().extract_videos);
        assert!(extractor.base_url().is_some());
    }

    #[test]
    fn test_convenience_functions() {
        assert!(has_any_media(TEST_HTML));

        let counts = count_all_media(TEST_HTML);
        assert!(counts.has_any());

        let urls = get_all_media_urls(TEST_HTML, None);
        assert!(!urls.is_empty());
    }

    #[test]
    fn test_extract_all_skips_noscript_iframe() {
        let html = r#"
            <noscript>
                <iframe src="https://www.googletagmanager.com/ns.html?id=GTM-532L8P"
                        height="0" width="0" style="display:none;visibility:hidden"></iframe>
            </noscript>
            <iframe src="https://example.com/visible"></iframe>
        "#;
        let extractor = MediaExtractor::new();
        let result = extractor.extract_all(html).unwrap();
        assert_eq!(result.embeds.len(), 1, "noscript 内的 iframe 不应被提取");
        assert_eq!(result.embeds[0].url, "https://example.com/visible");
    }
}
