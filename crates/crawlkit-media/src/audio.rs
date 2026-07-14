//! 音频提取
//!
//! 支持：
//! - HTML5 `<audio>` 元素
//! - 嵌入播放器（Spotify、SoundCloud、Apple Podcasts 等）
//! - 指向音频文件的链接（<a href>）

use scraper::{Html, Selector, ElementRef};
use std::collections::HashSet;
use url::Url;

use crate::types::{
    AudioMedia, AudioPlatform, AudioSource, MediaResult,
};

/// 音频文件扩展名列表
const AUDIO_EXTENSIONS: &[&str] = &[".mp3", ".wav", ".ogg", ".oga", ".flac", ".aac", ".m4a", ".opus", ".wma"];

/// 嵌入音频主机列表
const AUDIO_HOSTS: &[&str] = &[
    "open.spotify.com",
    "soundcloud.com",
    "w.soundcloud.com",
    "podcasts.apple.com",
    "anchor.fm",
    "podbean.com",
    "buzzsprout.com",
    "spreaker.com",
    "castbox.fm",
];

/// 从 HTML 文档提取所有音频
pub fn extract_audio(document: &Html, base_url: Option<&Url>) -> Vec<AudioMedia> {
    let mut audio_items = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    if let Ok(sel) = Selector::parse("audio") {
        for el in document.select(&sel) {
            if let Some(audio) = extract_audio_element(&el, base_url) {
                let key = audio.absolute_url.as_ref().unwrap_or(&audio.src).clone();
                if seen_urls.insert(key) {
                    audio_items.push(audio);
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("iframe[src]") {
        for el in document.select(&sel) {
            if let Some(src) = el.value().attr("src")
                && is_audio_embed(src)
                    && let Some(audio) = extract_embedded_audio(&el, base_url) {
                        let key = audio.absolute_url.as_ref().unwrap_or(&audio.src).clone();
                        if seen_urls.insert(key) {
                            audio_items.push(audio);
                        }
                    }
        }
    }

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href")
                && is_audio_file(href)
                    && let Some(audio) = create_audio_from_link(&el, base_url) {
                        let key = audio.absolute_url.as_ref().unwrap_or(&audio.src).clone();
                        if seen_urls.insert(key) {
                            audio_items.push(audio);
                        }
                    }
        }
    }

    audio_items
}

/// 提取 HTML5 audio 元素
fn extract_audio_element(el: &ElementRef, base_url: Option<&Url>) -> Option<AudioMedia> {
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

    let mut audio = AudioMedia {
        src: src.to_string(),
        absolute_url,
        platform: AudioPlatform::Html5,
        ..Default::default()
    };

    audio.autoplay = el.value().attr("autoplay").is_some();
    audio.loop_audio = el.value().attr("loop").is_some();
    audio.muted = el.value().attr("muted").is_some();
    audio.controls = el.value().attr("controls").is_some();

    audio.mime_type = el.value().attr("type").map(std::string::ToString::to_string)
        .or_else(|| guess_audio_mime(&audio.src));

    audio.sources = extract_audio_sources(el, base_url);

    audio.title = el.value().attr("title").map(std::string::ToString::to_string)
        .or_else(|| el.value().attr("aria-label").map(std::string::ToString::to_string));

    Some(audio)
}

/// 提取音频来源
fn extract_audio_sources(audio: &ElementRef, base_url: Option<&Url>) -> Vec<AudioSource> {
    let mut sources = Vec::new();

    if let Ok(sel) = Selector::parse("source") {
        for source in audio.select(&sel) {
            if let Some(src) = source.value().attr("src") {
                sources.push(AudioSource {
                    src: resolve_url(src, base_url).unwrap_or_else(|| src.to_string()),
                    mime_type: source.value().attr("type").map(std::string::ToString::to_string),
                });
            }
        }
    }

    sources
}

/// 提取嵌入音频
fn extract_embedded_audio(el: &ElementRef, base_url: Option<&Url>) -> Option<AudioMedia> {
    let src = el.value().attr("src")?;
    let platform = AudioPlatform::from_url(src);

    let mut audio = AudioMedia {
        src: src.to_string(),
        absolute_url: resolve_url(src, base_url),
        platform,
        embed_url: Some(src.to_string()),
        ..Default::default()
    };

    audio.title = el.value().attr("title").map(std::string::ToString::to_string);

    Some(audio)
}

/// 从链接创建音频
fn create_audio_from_link(el: &ElementRef, base_url: Option<&Url>) -> Option<AudioMedia> {
    let href = el.value().attr("href")?;

    Some(AudioMedia {
        src: href.to_string(),
        absolute_url: resolve_url(href, base_url),
        platform: AudioPlatform::Html5,
        title: Some(el.text().collect::<String>().trim().to_string()),
        mime_type: guess_audio_mime(href),
        ..Default::default()
    })
}

/// 判断是否为音频嵌入 URL
fn is_audio_embed(url: &str) -> bool {
    let u = url.to_lowercase();
    AUDIO_HOSTS.iter().any(|host| u.contains(host))
}

/// 判断是否为音频文件 URL
fn is_audio_file(url: &str) -> bool {
    let u = url.to_lowercase();
    AUDIO_EXTENSIONS.iter().any(|ext| u.ends_with(ext))
}

/// 从 URL 猜测音频 MIME 类型
fn guess_audio_mime(url: &str) -> Option<String> {
    let u = url.to_lowercase();

    if u.contains(".mp3") {
        Some("audio/mpeg".to_string())
    } else if u.contains(".wav") {
        Some("audio/wav".to_string())
    } else if u.contains(".ogg") || u.contains(".oga") {
        Some("audio/ogg".to_string())
    } else if u.contains(".flac") {
        Some("audio/flac".to_string())
    } else if u.contains(".aac") {
        Some("audio/aac".to_string())
    } else if u.contains(".m4a") {
        Some("audio/mp4".to_string())
    } else if u.contains(".opus") {
        Some("audio/opus".to_string())
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
        return Some(format!("https:{href}"));
    }

    base_url.and_then(|base| base.join(href).ok().map(|u| u.to_string()))
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 从 HTML 字符串提取音频
pub fn extract_audio_from_html(html: &str, base_url: Option<&str>) -> MediaResult<Vec<AudioMedia>> {
    let document = Html::parse_document(html);
    let base = base_url.and_then(|u| Url::parse(u).ok());
    Ok(extract_audio(&document, base.as_ref()))
}

/// 获取所有音频 URL
pub fn get_audio_urls(html: &str, base_url: Option<&str>) -> Vec<String> {
    extract_audio_from_html(html, base_url)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| a.absolute_url)
        .collect()
}

/// 检查 HTML 是否包含音频
pub fn has_audio(document: &Html) -> bool {
    if let Ok(sel) = Selector::parse("audio, iframe[src*='spotify'], iframe[src*='soundcloud']") {
        document.select(&sel).next().is_some()
    } else {
        false
    }
}

/// 获取 Spotify 嵌入 URL
pub fn spotify_embed_url(track_id: &str) -> String {
    format!("https://open.spotify.com/embed/track/{track_id}")
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
    fn test_extract_html5_audio() {
        let html = r#"<audio src="/audio/podcast.mp3" controls></audio>"#;
        let doc = parse_html(html);
        let base = Url::parse("https://example.com").unwrap();
        let audio = extract_audio(&doc, Some(&base));

        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].platform, AudioPlatform::Html5);
        assert!(audio[0].controls);
    }

    #[test]
    fn test_extract_spotify_embed() {
        let html = r#"<iframe src="https://open.spotify.com/embed/track/4iV5W9uYEdYUVa79Axb7Rh"></iframe>"#;
        let doc = parse_html(html);
        let audio = extract_audio(&doc, None);

        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].platform, AudioPlatform::Spotify);
    }

    #[test]
    fn test_extract_audio_link() {
        let html = r#"<a href="/downloads/song.mp3">Download Song</a>"#;
        let doc = parse_html(html);
        let audio = extract_audio(&doc, None);

        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].title, Some("Download Song".to_string()));
    }

    #[test]
    fn test_has_audio() {
        let with_audio = "<audio src='test.mp3'></audio>";
        let without = "<p>No audio</p>";

        assert!(has_audio(&parse_html(with_audio)));
        assert!(!has_audio(&parse_html(without)));
    }
}
