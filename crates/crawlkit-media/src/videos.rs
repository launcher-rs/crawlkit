//! 视频提取
//!
//! 支持：
//! - HTML5 `<video>` 元素
//! - 嵌入播放器（YouTube、Vimeo、Dailymotion、Twitch、TikTok 等）
//! - 视频来源（<source>）和轨道（<track>）
//! - 视频 ID 提取和缩略图生成

use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector, ElementRef};
use std::collections::HashSet;
use url::Url;

use crate::types::{
    VideoMedia, VideoPlatform, VideoSource, VideoTrack, TrackKind, MediaResult,
};

lazy_static! {
    static ref YOUTUBE_ID: Regex = Regex::new(
        r"(?:youtube\.com/(?:watch\?v=|embed/|v/)|youtu\.be/)([a-zA-Z0-9_-]{11})"
    ).unwrap();

    static ref VIMEO_ID: Regex = Regex::new(
        r"vimeo\.com/(?:video/)?(\d+)"
    ).unwrap();

    static ref DAILYMOTION_ID: Regex = Regex::new(
        r"dailymotion\.com/(?:video|embed/video)/([a-zA-Z0-9]+)"
    ).unwrap();

    static ref TWITCH_ID: Regex = Regex::new(
        r"twitch\.tv/(?:videos/(\d+)|([a-zA-Z0-9_]+))"
    ).unwrap();

    static ref TIKTOK_ID: Regex = Regex::new(
        r"tiktok\.com/@[^/]+/video/(\d+)"
    ).unwrap();
}

/// 从 HTML 文档提取所有视频
pub fn extract_videos(document: &Html, base_url: Option<&Url>) -> Vec<VideoMedia> {
    let mut videos = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    if let Ok(sel) = Selector::parse("video") {
        for el in document.select(&sel) {
            if let Some(video) = extract_video_element(&el, base_url) {
                let key = video.absolute_url.as_ref().unwrap_or(&video.src).clone();
                if seen_urls.insert(key) {
                    videos.push(video);
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("iframe[src]") {
        for el in document.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if is_video_embed(src) {
                    if let Some(video) = extract_embedded_video(&el, base_url) {
                        let key = video.absolute_url.as_ref().unwrap_or(&video.src).clone();
                        if seen_urls.insert(key) {
                            videos.push(video);
                        }
                    }
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("object[data], embed[src]") {
        for el in document.select(&sel) {
            let src = el.value().attr("data").or_else(|| el.value().attr("src"));
            if let Some(src) = src {
                if is_video_url(src) {
                    if let Some(video) = create_video_from_url(src, &el, base_url) {
                        let key = video.absolute_url.as_ref().unwrap_or(&video.src).clone();
                        if seen_urls.insert(key) {
                            videos.push(video);
                        }
                    }
                }
            }
        }
    }

    videos
}

/// 提取 HTML5 video 元素
fn extract_video_element(el: &ElementRef, base_url: Option<&Url>) -> Option<VideoMedia> {
    let src = el.value().attr("src")
        .or_else(|| {
            if let Ok(sel) = Selector::parse("source") {
                el.select(&sel).next()
                    .and_then(|s| s.value().attr("src"))
            } else {
                None
            }
        })?;

    let absolute_url = resolve_url(src, base_url);

    let mut video = VideoMedia {
        src: src.to_string(),
        absolute_url,
        platform: VideoPlatform::Html5,
        ..Default::default()
    };

    video.poster = el.value().attr("poster")
        .and_then(|p| resolve_url(p, base_url));
    video.width = el.value().attr("width")
        .and_then(|w| w.trim_end_matches("px").parse().ok());
    video.height = el.value().attr("height")
        .and_then(|h| h.trim_end_matches("px").parse().ok());
    video.autoplay = el.value().attr("autoplay").is_some();
    video.loop_video = el.value().attr("loop").is_some();
    video.muted = el.value().attr("muted").is_some();
    video.controls = el.value().attr("controls").is_some();
    video.playsinline = el.value().attr("playsinline").is_some();

    video.mime_type = el.value().attr("type").map(|s| s.to_string())
        .or_else(|| guess_video_mime(&video.src));

    video.sources = extract_video_sources(el, base_url);
    video.tracks = extract_video_tracks(el, base_url);

    video.title = el.value().attr("title").map(|s| s.to_string())
        .or_else(|| el.value().attr("aria-label").map(|s| s.to_string()));

    Some(video)
}

/// 提取视频来源
fn extract_video_sources(video: &ElementRef, base_url: Option<&Url>) -> Vec<VideoSource> {
    let mut sources = Vec::new();

    if let Ok(sel) = Selector::parse("source") {
        for source in video.select(&sel) {
            if let Some(src) = source.value().attr("src") {
                sources.push(VideoSource {
                    src: resolve_url(src, base_url).unwrap_or_else(|| src.to_string()),
                    mime_type: source.value().attr("type").map(|s| s.to_string()),
                    quality: source.value().attr("data-quality")
                        .or_else(|| source.value().attr("label"))
                        .map(|s| s.to_string()),
                });
            }
        }
    }

    sources
}

/// 提取视频轨道
fn extract_video_tracks(video: &ElementRef, base_url: Option<&Url>) -> Vec<VideoTrack> {
    let mut tracks = Vec::new();

    if let Ok(sel) = Selector::parse("track") {
        for track in video.select(&sel) {
            if let Some(src) = track.value().attr("src") {
                let kind = match track.value().attr("kind") {
                    Some("captions") => TrackKind::Captions,
                    Some("descriptions") => TrackKind::Descriptions,
                    Some("chapters") => TrackKind::Chapters,
                    Some("metadata") => TrackKind::Metadata,
                    _ => TrackKind::Subtitles,
                };

                tracks.push(VideoTrack {
                    src: resolve_url(src, base_url).unwrap_or_else(|| src.to_string()),
                    kind,
                    label: track.value().attr("label").map(|s| s.to_string()),
                    srclang: track.value().attr("srclang").map(|s| s.to_string()),
                    is_default: track.value().attr("default").is_some(),
                });
            }
        }
    }

    tracks
}

/// 提取嵌入视频
fn extract_embedded_video(el: &ElementRef, base_url: Option<&Url>) -> Option<VideoMedia> {
    let src = el.value().attr("src")?;
    let platform = VideoPlatform::from_url(src);

    let mut video = VideoMedia {
        src: src.to_string(),
        absolute_url: resolve_url(src, base_url),
        platform,
        embed_url: Some(src.to_string()),
        ..Default::default()
    };

    video.video_id = extract_video_id(src, platform);
    video.width = el.value().attr("width")
        .and_then(|w| w.trim_end_matches("px").parse().ok());
    video.height = el.value().attr("height")
        .and_then(|h| h.trim_end_matches("px").parse().ok());
    video.title = el.value().attr("title").map(|s| s.to_string());
    video.poster = generate_thumbnail_url(&video);

    Some(video)
}

/// 从 URL 创建视频
fn create_video_from_url(src: &str, el: &ElementRef, base_url: Option<&Url>) -> Option<VideoMedia> {
    let platform = if is_video_embed(src) {
        VideoPlatform::from_url(src)
    } else {
        VideoPlatform::Html5
    };

    let mut video = VideoMedia {
        src: src.to_string(),
        absolute_url: resolve_url(src, base_url),
        platform,
        ..Default::default()
    };

    video.video_id = extract_video_id(src, platform);
    video.width = el.value().attr("width")
        .and_then(|w| w.parse().ok());
    video.height = el.value().attr("height")
        .and_then(|h| h.parse().ok());

    Some(video)
}

/// 根据平台提取视频 ID
fn extract_video_id(url: &str, platform: VideoPlatform) -> Option<String> {
    match platform {
        VideoPlatform::YouTube => {
            YOUTUBE_ID.captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        }
        VideoPlatform::Vimeo => {
            VIMEO_ID.captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        }
        VideoPlatform::Dailymotion => {
            DAILYMOTION_ID.captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        }
        VideoPlatform::Twitch => {
            TWITCH_ID.captures(url)
                .and_then(|c| c.get(1).or_else(|| c.get(2)))
                .map(|m| m.as_str().to_string())
        }
        VideoPlatform::TikTok => {
            TIKTOK_ID.captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        }
        _ => None,
    }
}

/// 生成缩略图 URL
fn generate_thumbnail_url(video: &VideoMedia) -> Option<String> {
    match video.platform {
        VideoPlatform::YouTube => {
            video.video_id.as_ref().map(|id| {
                format!("https://img.youtube.com/vi/{}/maxresdefault.jpg", id)
            })
        }
        VideoPlatform::Dailymotion => {
            video.video_id.as_ref().map(|id| {
                format!("https://www.dailymotion.com/thumbnail/video/{}", id)
            })
        }
        _ => None,
    }
}

/// 判断是否为视频嵌入 URL
fn is_video_embed(url: &str) -> bool {
    let u = url.to_lowercase();
    let video_hosts = [
        "youtube.com", "youtube-nocookie.com", "youtu.be",
        "vimeo.com", "player.vimeo.com",
        "dailymotion.com", "dai.ly",
        "twitch.tv", "player.twitch.tv",
        "facebook.com/plugins/video",
        "tiktok.com",
        "wistia.com", "wistia.net",
        "brightcove",
        "jwplayer", "jwplatform",
    ];

    video_hosts.iter().any(|host| u.contains(host))
}

/// 判断是否为视频 URL
fn is_video_url(url: &str) -> bool {
    let u = url.to_lowercase();
    let video_extensions = [".mp4", ".webm", ".ogg", ".ogv", ".avi", ".mov", ".mkv", ".m4v"];

    video_extensions.iter().any(|ext| u.contains(ext))
        || is_video_embed(url)
}

/// 从 URL 猜测视频 MIME 类型
fn guess_video_mime(url: &str) -> Option<String> {
    let u = url.to_lowercase();

    if u.contains(".mp4") {
        Some("video/mp4".to_string())
    } else if u.contains(".webm") {
        Some("video/webm".to_string())
    } else if u.contains(".ogg") || u.contains(".ogv") {
        Some("video/ogg".to_string())
    } else if u.contains(".mov") {
        Some("video/quicktime".to_string())
    } else if u.contains(".avi") {
        Some("video/x-msvideo".to_string())
    } else {
        None
    }
}

/// 解析相对 URL
fn resolve_url(href: &str, base_url: Option<&Url>) -> Option<String> {
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }

    if href.starts_with("//") {
        return Some(format!("https:{}", href));
    }

    base_url.and_then(|base| base.join(href).ok().map(|u| u.to_string()))
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 从 HTML 字符串提取视频
pub fn extract_videos_from_html(html: &str, base_url: Option<&str>) -> MediaResult<Vec<VideoMedia>> {
    let document = Html::parse_document(html);
    let base = base_url.and_then(|u| Url::parse(u).ok());
    Ok(extract_videos(&document, base.as_ref()))
}

/// 获取所有视频 URL
pub fn get_video_urls(html: &str, base_url: Option<&str>) -> Vec<String> {
    extract_videos_from_html(html, base_url)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.absolute_url)
        .collect()
}

/// 检查 HTML 是否包含视频
pub fn has_videos(document: &Html) -> bool {
    if let Ok(sel) = Selector::parse("video, iframe[src*='youtube'], iframe[src*='vimeo']") {
        document.select(&sel).next().is_some()
    } else {
        false
    }
}

/// 获取 YouTube 嵌入 URL
pub fn youtube_embed_url(video_id: &str) -> String {
    format!("https://www.youtube.com/embed/{}", video_id)
}

/// 获取 Vimeo 嵌入 URL
pub fn vimeo_embed_url(video_id: &str) -> String {
    format!("https://player.vimeo.com/video/{}", video_id)
}

/// 获取 YouTube 缩略图 URL
pub fn youtube_thumbnail(video_id: &str, quality: &str) -> String {
    format!("https://img.youtube.com/vi/{}/{}.jpg", video_id, quality)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_html(html: &str) -> Html {
        Html::parse_document(html)
    }

    #[test]
    fn test_extract_html5_video() {
        let html = r#"<video src="/videos/test.mp4" width="640" height="360" controls></video>"#;
        let doc = parse_html(html);
        let base = Url::parse("https://example.com").unwrap();
        let videos = extract_videos(&doc, Some(&base));

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].platform, VideoPlatform::Html5);
        assert!(videos[0].controls);
        assert_eq!(videos[0].width, Some(640));
    }

    #[test]
    fn test_extract_youtube_embed() {
        let html = r#"
            <iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ"
                    width="560" height="315" title="Test Video">
            </iframe>
        "#;
        let doc = parse_html(html);
        let videos = extract_videos(&doc, None);

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].platform, VideoPlatform::YouTube);
        assert_eq!(videos[0].video_id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_has_videos() {
        let with_video = "<video src='test.mp4'></video>";
        let without = "<p>No video</p>";

        assert!(has_videos(&parse_html(with_video)));
        assert!(!has_videos(&parse_html(without)));
    }
}
