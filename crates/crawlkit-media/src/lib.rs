//! # crawlkit-media
//!
//! 媒体提取与下载模块。
//!
//! 从 HTML 文档中提取各类媒体资源：
//! - **图片**：支持 srcset、懒加载、占位图检测
//! - **视频**：HTML5 视频和嵌入平台（YouTube、Vimeo 等）
//! - **音频**：HTML5 音频和流媒体平台（Spotify、SoundCloud 等）
//! - **文档**：PDF、Office 文档、EPUB 等
//! - **嵌入内容**：iframe、社交嵌入、地图、小部件
//!
//! 同时提供可选的异步下载器，支持并发下载、重试和哈希验证。
//!
//! ## 快速开始
//!
//! ```rust,no_run
//! use crawlkit_media::MediaExtractor;
//!
//! let html = r#"
//!     <img src="image.jpg" alt="Photo">
//!     <video src="video.mp4"></video>
//! "#;
//!
//! let extractor = MediaExtractor::new()
//!     .with_base_url("https://example.com");
//!
//! let media = extractor.extract_all(html).unwrap();
//! println!("找到 {} 张图片", media.images.len());
//! println!("找到 {} 个视频", media.videos.len());
//! ```

// ============================================================================
// 模块声明
// ============================================================================

pub mod types;
pub mod images;
pub mod videos;
pub mod audio;
pub mod documents;
pub mod embedded;
pub mod extractor;

#[cfg(feature = "downloader")]
pub mod downloader;

// ============================================================================
// 公共重导出
// ============================================================================

pub use types::{
    MediaError, MediaResult,
    MediaType, ImageFormat, ImageLoading,
    ImageMedia, SrcsetEntry,
    VideoMedia, VideoSource, VideoTrack, TrackKind, VideoPlatform,
    AudioMedia, AudioSource, AudioPlatform,
    DocumentMedia, DocumentType,
    EmbeddedMedia, EmbedPlatform, EmbedType,
    LinkMedia, LinkType,
    MediaConfig, DownloadConfig, DownloadResult,
    ExtractedMedia,
};

pub use images::{
    extract_images, get_image_urls, has_images,
    filter_placeholders, get_best_image_url,
};

pub use videos::{
    extract_videos, get_video_urls, has_videos,
    youtube_thumbnail, youtube_embed_url,
};

pub use audio::{
    extract_audio, get_audio_urls, has_audio,
    spotify_embed_url,
};

pub use documents::{
    extract_documents, get_document_urls, has_documents,
    get_pdfs, get_office_docs,
};

pub use embedded::{
    extract_embeds, get_embed_urls, has_embeds,
    detect_embed_platform, filter_by_platform,
    get_maps, get_social_embeds, get_code_embeds,
};

pub use extractor::{
    MediaExtractor, MediaExtractorBuilder, MediaCounts,
    extract_media, has_any_media, count_all_media, get_all_media_urls,
};

#[cfg(feature = "downloader")]
pub use downloader::{
    MediaDownloader,
    download_bytes, download_with_hash, download_to_base64,
    save_to_file, compute_sha256, detect_media_type,
    url_to_filename, is_downloadable,
};
