//! 核心类型定义
//!
//! 包含所有媒体提取相关类型：错误、媒体枚举、图片/视频/音频/文档/嵌入类型、配置

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// 错误类型
// ============================================================================

/// 媒体操作错误类型
#[derive(Error, Debug)]
pub enum MediaError {
    #[error("下载媒体失败: {0}")]
    Download(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("HTTP 错误 {0}: {1}")]
    Http(u16, String),

    #[error("无效 URL: {0}")]
    InvalidUrl(String),

    #[error("不支持的媒体类型: {0}")]
    UnsupportedType(String),

    #[error("文件过大: {0} 字节 (最大: {1})")]
    FileTooLarge(u64, u64),

    #[error("下载超时: {0}")]
    Timeout(String),

    #[error("IO 错误: {0}")]
    Io(String),

    #[error("解析错误: {0}")]
    Parse(String),
}

/// 媒体操作结果类型
pub type MediaResult<T> = Result<T, MediaError>;

// ============================================================================
// 媒体类型枚举
// ============================================================================

/// 媒体类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    Embedded,
    Other,
}

impl MediaType {
    /// 从文件扩展名推断媒体类型
    pub fn from_extension(ext: &str) -> Self {
        let ext = ext.to_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "ico" | "bmp"
            | "avif" | "heic" | "heif" | "tiff" | "tif" => MediaType::Image,

            "mp4" | "webm" | "ogg" | "ogv" | "avi" | "mov" | "mkv" | "m4v"
            | "wmv" | "flv" | "3gp" => MediaType::Video,

            "mp3" | "wav" | "oga" | "flac" | "aac" | "m4a" | "wma"
            | "opus" | "aiff" => MediaType::Audio,

            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
            | "txt" | "rtf" | "odt" | "ods" | "odp" | "csv" | "epub" => MediaType::Document,

            _ => MediaType::Other,
        }
    }

    /// 从 MIME 类型推断媒体类型
    pub fn from_mime(mime: &str) -> Self {
        let m = mime.to_lowercase();
        if m.starts_with("image/") {
            MediaType::Image
        } else if m.starts_with("video/") {
            MediaType::Video
        } else if m.starts_with("audio/") {
            MediaType::Audio
        } else if m.starts_with("application/pdf")
            || m.contains("document")
            || m.contains("spreadsheet")
            || m.contains("presentation")
        {
            MediaType::Document
        } else {
            MediaType::Other
        }
    }
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Image => write!(f, "image"),
            MediaType::Video => write!(f, "video"),
            MediaType::Audio => write!(f, "audio"),
            MediaType::Document => write!(f, "document"),
            MediaType::Embedded => write!(f, "embedded"),
            MediaType::Other => write!(f, "other"),
        }
    }
}

// ============================================================================
// 图片类型
// ============================================================================

/// 图片格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
    Svg,
    Avif,
    Heic,
    Ico,
    Bmp,
    Tiff,
    Unknown,
}

impl ImageFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "png" => ImageFormat::Png,
            "gif" => ImageFormat::Gif,
            "webp" => ImageFormat::WebP,
            "svg" => ImageFormat::Svg,
            "avif" => ImageFormat::Avif,
            "heic" | "heif" => ImageFormat::Heic,
            "ico" => ImageFormat::Ico,
            "bmp" => ImageFormat::Bmp,
            "tiff" | "tif" => ImageFormat::Tiff,
            _ => ImageFormat::Unknown,
        }
    }

    pub fn from_mime(mime: &str) -> Self {
        match mime.to_lowercase().as_str() {
            "image/jpeg" => ImageFormat::Jpeg,
            "image/png" => ImageFormat::Png,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::WebP,
            "image/svg+xml" => ImageFormat::Svg,
            "image/avif" => ImageFormat::Avif,
            "image/heic" | "image/heif" => ImageFormat::Heic,
            "image/x-icon" | "image/vnd.microsoft.icon" => ImageFormat::Ico,
            "image/bmp" => ImageFormat::Bmp,
            "image/tiff" => ImageFormat::Tiff,
            _ => ImageFormat::Unknown,
        }
    }

    pub const fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Avif => "image/avif",
            ImageFormat::Heic => "image/heic",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Unknown => "application/octet-stream",
        }
    }
}

/// 图片加载策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageLoading {
    #[default]
    Eager,
    Lazy,
}

/// Srcset 条目（响应式图片）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SrcsetEntry {
    pub url: String,
    pub width: Option<u32>,
    pub density: Option<f32>,
}

/// 提取的图片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMedia {
    pub src: String,
    pub absolute_url: Option<String>,
    pub alt: Option<String>,
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: ImageFormat,
    pub mime_type: Option<String>,
    pub loading: ImageLoading,
    pub is_decorative: bool,
    pub srcset: Vec<SrcsetEntry>,
    pub sizes: Option<String>,
    pub data_src: Option<String>,
    pub is_placeholder: bool,
    pub size_bytes: Option<usize>,
    pub content_hash: Option<String>,
    pub classes: Vec<String>,
    pub id: Option<String>,
}

impl Default for ImageMedia {
    fn default() -> Self {
        Self {
            src: String::new(),
            absolute_url: None,
            alt: None,
            title: None,
            width: None,
            height: None,
            format: ImageFormat::Unknown,
            mime_type: None,
            loading: ImageLoading::Eager,
            is_decorative: false,
            srcset: Vec::new(),
            sizes: None,
            data_src: None,
            is_placeholder: false,
            size_bytes: None,
            content_hash: None,
            classes: Vec::new(),
            id: None,
        }
    }
}

// ============================================================================
// 视频类型
// ============================================================================

/// 视频平台（嵌入视频）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoPlatform {
    YouTube,
    Vimeo,
    Dailymotion,
    Twitch,
    Facebook,
    Twitter,
    TikTok,
    Wistia,
    Brightcove,
    JWPlayer,
    VideoJs,
    Html5,
    Other,
}

impl VideoPlatform {
    pub fn from_url(url: &str) -> Self {
        let u = url.to_lowercase();
        if u.contains("youtube.com") || u.contains("youtu.be") {
            VideoPlatform::YouTube
        } else if u.contains("vimeo.com") {
            VideoPlatform::Vimeo
        } else if u.contains("dailymotion.com") || u.contains("dai.ly") {
            VideoPlatform::Dailymotion
        } else if u.contains("twitch.tv") {
            VideoPlatform::Twitch
        } else if u.contains("facebook.com") || u.contains("fb.watch") {
            VideoPlatform::Facebook
        } else if u.contains("twitter.com") || u.contains("x.com") {
            VideoPlatform::Twitter
        } else if u.contains("tiktok.com") {
            VideoPlatform::TikTok
        } else if u.contains("wistia.com") || u.contains("wistia.net") {
            VideoPlatform::Wistia
        } else if u.contains("brightcove") {
            VideoPlatform::Brightcove
        } else if u.contains("jwplayer") || u.contains("jwplatform") {
            VideoPlatform::JWPlayer
        } else {
            VideoPlatform::Other
        }
    }
}

/// 提取的视频
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMedia {
    pub src: String,
    pub absolute_url: Option<String>,
    pub platform: VideoPlatform,
    pub video_id: Option<String>,
    pub poster: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    pub mime_type: Option<String>,
    pub title: Option<String>,
    pub sources: Vec<VideoSource>,
    pub tracks: Vec<VideoTrack>,
    pub autoplay: bool,
    pub loop_video: bool,
    pub muted: bool,
    pub controls: bool,
    pub playsinline: bool,
    pub embed_url: Option<String>,
    pub size_bytes: Option<usize>,
}

impl Default for VideoMedia {
    fn default() -> Self {
        Self {
            src: String::new(),
            absolute_url: None,
            platform: VideoPlatform::Html5,
            video_id: None,
            poster: None,
            width: None,
            height: None,
            duration: None,
            mime_type: None,
            title: None,
            sources: Vec::new(),
            tracks: Vec::new(),
            autoplay: false,
            loop_video: false,
            muted: false,
            controls: true,
            playsinline: false,
            embed_url: None,
            size_bytes: None,
        }
    }
}

/// 视频来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSource {
    pub src: String,
    pub mime_type: Option<String>,
    pub quality: Option<String>,
}

/// 视频轨道（字幕、说明）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTrack {
    pub src: String,
    pub kind: TrackKind,
    pub label: Option<String>,
    pub srclang: Option<String>,
    pub is_default: bool,
}

/// 轨道类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    #[default]
    Subtitles,
    Captions,
    Descriptions,
    Chapters,
    Metadata,
}

// ============================================================================
// 音频类型
// ============================================================================

/// 音频平台
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioPlatform {
    Spotify,
    SoundCloud,
    ApplePodcasts,
    Anchor,
    Podbean,
    Buzzsprout,
    Html5,
    Other,
}

impl AudioPlatform {
    pub fn from_url(url: &str) -> Self {
        let u = url.to_lowercase();
        if u.contains("spotify.com") || u.contains("open.spotify") {
            AudioPlatform::Spotify
        } else if u.contains("soundcloud.com") {
            AudioPlatform::SoundCloud
        } else if u.contains("podcasts.apple.com") {
            AudioPlatform::ApplePodcasts
        } else if u.contains("anchor.fm") {
            AudioPlatform::Anchor
        } else if u.contains("podbean.com") {
            AudioPlatform::Podbean
        } else if u.contains("buzzsprout.com") {
            AudioPlatform::Buzzsprout
        } else {
            AudioPlatform::Other
        }
    }
}

/// 提取的音频
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMedia {
    pub src: String,
    pub absolute_url: Option<String>,
    pub platform: AudioPlatform,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub mime_type: Option<String>,
    pub sources: Vec<AudioSource>,
    pub autoplay: bool,
    pub loop_audio: bool,
    pub muted: bool,
    pub controls: bool,
    pub embed_url: Option<String>,
    pub size_bytes: Option<usize>,
}

impl Default for AudioMedia {
    fn default() -> Self {
        Self {
            src: String::new(),
            absolute_url: None,
            platform: AudioPlatform::Html5,
            title: None,
            artist: None,
            album: None,
            duration: None,
            mime_type: None,
            sources: Vec::new(),
            autoplay: false,
            loop_audio: false,
            muted: false,
            controls: true,
            embed_url: None,
            size_bytes: None,
        }
    }
}

/// 音频来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSource {
    pub src: String,
    pub mime_type: Option<String>,
}

// ============================================================================
// 文档类型
// ============================================================================

/// 文档类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentType {
    Pdf,
    Word,
    Excel,
    PowerPoint,
    Text,
    Csv,
    Epub,
    Other,
}

impl DocumentType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "pdf" => DocumentType::Pdf,
            "doc" | "docx" | "odt" | "rtf" => DocumentType::Word,
            "xls" | "xlsx" | "ods" => DocumentType::Excel,
            "ppt" | "pptx" | "odp" => DocumentType::PowerPoint,
            "txt" => DocumentType::Text,
            "csv" => DocumentType::Csv,
            "epub" => DocumentType::Epub,
            _ => DocumentType::Other,
        }
    }
}

/// 提取的文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMedia {
    pub url: String,
    pub absolute_url: Option<String>,
    pub doc_type: DocumentType,
    pub filename: Option<String>,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<usize>,
    pub page_count: Option<u32>,
}

impl Default for DocumentMedia {
    fn default() -> Self {
        Self {
            url: String::new(),
            absolute_url: None,
            doc_type: DocumentType::Other,
            filename: None,
            title: None,
            mime_type: None,
            size_bytes: None,
            page_count: None,
        }
    }
}

// ============================================================================
// 嵌入内容类型
// ============================================================================

/// 嵌入内容类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbedType {
    Iframe,
    Object,
    Embed,
    Script,
}

/// 嵌入平台
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbedPlatform {
    YouTube,
    Vimeo,
    Dailymotion,
    Twitch,
    Wistia,
    Twitter,
    Instagram,
    Facebook,
    LinkedIn,
    Pinterest,
    TikTok,
    Reddit,
    Spotify,
    SoundCloud,
    ApplePodcasts,
    GoogleMaps,
    GoogleDocs,
    CodePen,
    JsFiddle,
    CodeSandbox,
    Gist,
    SlideShare,
    Giphy,
    Typeform,
    Calendly,
    Stripe,
    PayPal,
    Scribd,
    Other,
}

impl EmbedPlatform {
    pub fn from_url(url: &str) -> Self {
        let u = url.to_lowercase();
        if u.contains("youtube.com") || u.contains("youtube-nocookie.com") {
            EmbedPlatform::YouTube
        } else if u.contains("player.vimeo.com") || u.contains("vimeo.com") {
            EmbedPlatform::Vimeo
        } else if u.contains("dailymotion.com") {
            EmbedPlatform::Dailymotion
        } else if u.contains("twitch.tv") {
            EmbedPlatform::Twitch
        } else if u.contains("wistia.com") || u.contains("wistia.net") {
            EmbedPlatform::Wistia
        } else if u.contains("platform.twitter.com") || u.contains("twitter.com/") || u.contains("x.com") {
            EmbedPlatform::Twitter
        } else if u.contains("instagram.com") {
            EmbedPlatform::Instagram
        } else if u.contains("facebook.com") || u.contains("fb.com") {
            EmbedPlatform::Facebook
        } else if u.contains("linkedin.com") {
            EmbedPlatform::LinkedIn
        } else if u.contains("pinterest.com") {
            EmbedPlatform::Pinterest
        } else if u.contains("tiktok.com") {
            EmbedPlatform::TikTok
        } else if u.contains("reddit.com") || u.contains("redd.it") {
            EmbedPlatform::Reddit
        } else if u.contains("open.spotify.com") || u.contains("spotify.com") {
            EmbedPlatform::Spotify
        } else if u.contains("soundcloud.com") {
            EmbedPlatform::SoundCloud
        } else if u.contains("podcasts.apple.com") {
            EmbedPlatform::ApplePodcasts
        } else if u.contains("google.com/maps") || u.contains("maps.google") {
            EmbedPlatform::GoogleMaps
        } else if u.contains("docs.google.com") {
            EmbedPlatform::GoogleDocs
        } else if u.contains("codepen.io") {
            EmbedPlatform::CodePen
        } else if u.contains("jsfiddle.net") {
            EmbedPlatform::JsFiddle
        } else if u.contains("codesandbox.io") {
            EmbedPlatform::CodeSandbox
        } else if u.contains("gist.github.com") {
            EmbedPlatform::Gist
        } else if u.contains("slideshare.net") {
            EmbedPlatform::SlideShare
        } else if u.contains("giphy.com") {
            EmbedPlatform::Giphy
        } else if u.contains("typeform.com") {
            EmbedPlatform::Typeform
        } else if u.contains("calendly.com") {
            EmbedPlatform::Calendly
        } else if u.contains("stripe.com") {
            EmbedPlatform::Stripe
        } else if u.contains("paypal.com") {
            EmbedPlatform::PayPal
        } else if u.contains("scribd.com") {
            EmbedPlatform::Scribd
        } else {
            EmbedPlatform::Other
        }
    }
}

/// 提取的嵌入内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedMedia {
    pub url: String,
    pub absolute_url: Option<String>,
    pub platform: EmbedPlatform,
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub allow: Option<String>,
    pub sandbox: Option<String>,
    pub loading: Option<String>,
    pub frameborder: Option<String>,
}

impl Default for EmbeddedMedia {
    fn default() -> Self {
        Self {
            url: String::new(),
            absolute_url: None,
            platform: EmbedPlatform::Other,
            title: None,
            width: None,
            height: None,
            allow: None,
            sandbox: None,
            loading: None,
            frameborder: None,
        }
    }
}

// ============================================================================
// 链接类型
// ============================================================================

/// 链接类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    #[default]
    Internal,
    External,
    Mailto,
    Tel,
    Download,
    Anchor,
}

/// 提取的链接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkMedia {
    pub href: String,
    pub absolute_url: Option<String>,
    pub text: String,
    pub title: Option<String>,
    pub rel: Vec<String>,
    pub link_type: LinkType,
    pub is_nofollow: bool,
    pub is_sponsored: bool,
    pub is_ugc: bool,
    pub target: Option<String>,
    pub download: Option<String>,
    pub hreflang: Option<String>,
    pub media_type: Option<MediaType>,
}

impl Default for LinkMedia {
    fn default() -> Self {
        Self {
            href: String::new(),
            absolute_url: None,
            text: String::new(),
            title: None,
            rel: Vec::new(),
            link_type: LinkType::Internal,
            is_nofollow: false,
            is_sponsored: false,
            is_ugc: false,
            target: None,
            download: None,
            hreflang: None,
            media_type: None,
        }
    }
}

// ============================================================================
// 配置
// ============================================================================

/// 媒体提取配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    pub extract_images: bool,
    pub extract_videos: bool,
    pub extract_audio: bool,
    pub extract_documents: bool,
    pub extract_embeds: bool,
    pub extract_links: bool,
    pub include_data_urls: bool,
    pub filter_placeholders: bool,
    pub min_image_width: Option<u32>,
    pub min_image_height: Option<u32>,
    pub download: DownloadConfig,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            extract_images: true,
            extract_videos: true,
            extract_audio: true,
            extract_documents: true,
            extract_embeds: true,
            extract_links: true,
            include_data_urls: false,
            filter_placeholders: true,
            min_image_width: None,
            min_image_height: None,
            download: DownloadConfig::default(),
        }
    }
}

impl MediaConfig {
    /// 创建最小配置（仅图片和链接）
    pub fn minimal() -> Self {
        Self {
            extract_images: true,
            extract_videos: false,
            extract_audio: false,
            extract_documents: false,
            extract_embeds: false,
            extract_links: true,
            ..Default::default()
        }
    }

    /// 创建完整配置（提取全部）
    pub fn full() -> Self {
        Self {
            extract_images: true,
            extract_videos: true,
            extract_audio: true,
            extract_documents: true,
            extract_embeds: true,
            extract_links: true,
            include_data_urls: true,
            ..Default::default()
        }
    }
}

/// 下载配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub max_file_size: Option<u64>,
    pub max_concurrent: usize,
    pub timeout_secs: u64,
    pub encode_base64: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub user_agent: String,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_file_size: Some(50 * 1024 * 1024),
            max_concurrent: 10,
            timeout_secs: 30,
            encode_base64: false,
            max_retries: 2,
            retry_delay_ms: 1000,
            user_agent: "crawlkit-media/0.1".to_string(),
        }
    }
}

/// 下载结果
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub url: String,
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub size: u64,
    pub hash: String,
    pub media_type: MediaType,
    pub base64: Option<String>,
}

// ============================================================================
// 提取结果集合
// ============================================================================

/// 从页面提取的所有媒体
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedMedia {
    pub images: Vec<ImageMedia>,
    pub videos: Vec<VideoMedia>,
    pub audio: Vec<AudioMedia>,
    pub documents: Vec<DocumentMedia>,
    pub embeds: Vec<EmbeddedMedia>,
    pub links: Vec<LinkMedia>,
}

impl ExtractedMedia {
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取媒体总数
    pub fn total_count(&self) -> usize {
        self.images.len()
            + self.videos.len()
            + self.audio.len()
            + self.documents.len()
            + self.embeds.len()
            + self.links.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    /// 是否包含媒体
    pub fn has_media(&self) -> bool {
        !self.is_empty()
    }

    /// 获取所有媒体 URL
    pub fn all_urls(&self) -> Vec<String> {
        let mut urls = Vec::new();

        for img in &self.images {
            if let Some(url) = &img.absolute_url {
                urls.push(url.clone());
            }
        }

        for vid in &self.videos {
            if let Some(url) = &vid.absolute_url {
                urls.push(url.clone());
            }
        }

        for aud in &self.audio {
            if let Some(url) = &aud.absolute_url {
                urls.push(url.clone());
            }
        }

        for doc in &self.documents {
            if let Some(url) = &doc.absolute_url {
                urls.push(url.clone());
            }
        }

        for emb in &self.embeds {
            if let Some(url) = &emb.absolute_url {
                urls.push(url.clone());
            }
        }

        urls
    }

    /// 获取所有图片 URL
    pub fn image_urls(&self) -> Vec<&str> {
        self.images.iter()
            .filter_map(|i| i.absolute_url.as_deref())
            .collect()
    }

    /// 获取所有视频 URL
    pub fn video_urls(&self) -> Vec<&str> {
        self.videos.iter()
            .filter_map(|v| v.absolute_url.as_deref())
            .collect()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_type_from_extension() {
        assert_eq!(MediaType::from_extension("jpg"), MediaType::Image);
        assert_eq!(MediaType::from_extension("PNG"), MediaType::Image);
        assert_eq!(MediaType::from_extension("mp4"), MediaType::Video);
        assert_eq!(MediaType::from_extension("mp3"), MediaType::Audio);
        assert_eq!(MediaType::from_extension("pdf"), MediaType::Document);
        assert_eq!(MediaType::from_extension("xyz"), MediaType::Other);
    }

    #[test]
    fn test_media_type_from_mime() {
        assert_eq!(MediaType::from_mime("image/jpeg"), MediaType::Image);
        assert_eq!(MediaType::from_mime("video/mp4"), MediaType::Video);
        assert_eq!(MediaType::from_mime("audio/mpeg"), MediaType::Audio);
        assert_eq!(MediaType::from_mime("application/pdf"), MediaType::Document);
    }

    #[test]
    fn test_image_format() {
        assert_eq!(ImageFormat::from_extension("jpg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_extension("webp"), ImageFormat::WebP);
        assert_eq!(ImageFormat::from_mime("image/png"), ImageFormat::Png);
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
    }

    #[test]
    fn test_video_platform_detection() {
        assert_eq!(VideoPlatform::from_url("https://youtube.com/watch?v=abc"), VideoPlatform::YouTube);
        assert_eq!(VideoPlatform::from_url("https://vimeo.com/123"), VideoPlatform::Vimeo);
        assert_eq!(VideoPlatform::from_url("https://twitch.tv/channel"), VideoPlatform::Twitch);
        assert_eq!(VideoPlatform::from_url("https://example.com/video.mp4"), VideoPlatform::Other);
    }

    #[test]
    fn test_extracted_media() {
        let mut media = ExtractedMedia::new();
        assert!(!media.has_media());
        assert_eq!(media.total_count(), 0);

        media.images.push(ImageMedia::default());
        assert!(media.has_media());
        assert_eq!(media.total_count(), 1);
    }

    #[test]
    fn test_media_config() {
        let config = MediaConfig::default();
        assert!(config.extract_images);
        assert!(config.download.max_concurrent > 0);

        let minimal = MediaConfig::minimal();
        assert!(minimal.extract_images);
        assert!(!minimal.extract_videos);
    }
}
