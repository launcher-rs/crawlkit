//! 嵌入内容提取
//!
//! 支持：
//! - `<iframe>` 嵌入（地图、社交、小部件）
//! - `<object>` 和 `<embed>` 元素
//! - 社交嵌入（Twitter、Instagram、Facebook、Reddit）
//! - 平台检测（Google Maps、CodePen、Typeform、Calendly 等）

use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector, ElementRef, Node};
use std::collections::HashSet;
use url::Url;

use crate::types::{
    EmbeddedMedia, EmbedPlatform, MediaResult,
};

lazy_static! {
    static ref GOOGLE_MAPS: Regex = Regex::new(r"google\.com/maps|maps\.google\.").unwrap();
    static ref TWITTER: Regex = Regex::new(r"twitter\.com|x\.com|platform\.twitter").unwrap();
    static ref INSTAGRAM: Regex = Regex::new(r"instagram\.com").unwrap();
    static ref FACEBOOK: Regex = Regex::new(r"facebook\.com|fb\.com").unwrap();
    static ref LINKEDIN: Regex = Regex::new(r"linkedin\.com").unwrap();
    static ref PINTEREST: Regex = Regex::new(r"pinterest\.com").unwrap();
    static ref TIKTOK: Regex = Regex::new(r"tiktok\.com").unwrap();
    static ref REDDIT: Regex = Regex::new(r"reddit\.com|redd\.it").unwrap();
    static ref CODEPEN: Regex = Regex::new(r"codepen\.io").unwrap();
    static ref JSFIDDLE: Regex = Regex::new(r"jsfiddle\.net").unwrap();
    static ref CODESANDBOX: Regex = Regex::new(r"codesandbox\.io").unwrap();
    static ref GIPHY: Regex = Regex::new(r"giphy\.com").unwrap();
    static ref SLIDESHARE: Regex = Regex::new(r"slideshare\.net").unwrap();
    static ref TYPEFORM: Regex = Regex::new(r"typeform\.com").unwrap();
    static ref CALENDLY: Regex = Regex::new(r"calendly\.com").unwrap();
    static ref STRIPE: Regex = Regex::new(r"stripe\.com").unwrap();
    static ref PAYPAL: Regex = Regex::new(r"paypal\.com").unwrap();
}

/// 从 HTML 文档提取所有嵌入内容
pub fn extract_embeds(document: &Html, base_url: Option<&Url>) -> Vec<EmbeddedMedia> {
    let mut embeds = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    if let Ok(sel) = Selector::parse("iframe[src]") {
        for el in document.select(&sel) {
            if is_within_noscript(&el) {
                continue;
            }
            if let Some(embed) = extract_iframe(&el, base_url) {
                let key = embed.absolute_url.as_ref().unwrap_or(&embed.url).clone();
                if seen_urls.insert(key) {
                    embeds.push(embed);
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("object[data]") {
        for el in document.select(&sel) {
            if is_within_noscript(&el) {
                continue;
            }
            if let Some(embed) = extract_object(&el, base_url) {
                let key = embed.absolute_url.as_ref().unwrap_or(&embed.url).clone();
                if seen_urls.insert(key) {
                    embeds.push(embed);
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("embed[src]") {
        for el in document.select(&sel) {
            if is_within_noscript(&el) {
                continue;
            }
            if let Some(embed) = extract_embed_tag(&el, base_url) {
                let key = embed.absolute_url.as_ref().unwrap_or(&embed.url).clone();
                if seen_urls.insert(key) {
                    embeds.push(embed);
                }
            }
        }
    }

    extract_social_embeds(document, &mut embeds, &mut seen_urls);

    embeds
}

/// 检查元素是否位于 <noscript> 标签内
pub(crate) fn is_within_noscript(el: &ElementRef) -> bool {
    let mut current = el.parent();
    while let Some(p) = current {
        if let Node::Element(element) = p.value() {
            if element.name() == "noscript" {
                return true;
            }
        }
        current = p.parent();
    }
    false
}

/// 提取 iframe 元素
fn extract_iframe(el: &ElementRef, base_url: Option<&Url>) -> Option<EmbeddedMedia> {
    let src = el.value().attr("src")?;

    if src.is_empty() || src.starts_with("javascript:") || src.starts_with("about:") {
        return None;
    }

    let platform = detect_embed_platform(src);

    if is_video_or_audio_platform(&platform) {
        return None;
    }

    let width = el.value().attr("width")
        .and_then(parse_dimension);
    let height = el.value().attr("height")
        .and_then(parse_dimension);

    Some(EmbeddedMedia {
        url: src.to_string(),
        absolute_url: resolve_url(src, base_url),
        platform,
        title: el.value().attr("title").map(|s| s.to_string()),
        width,
        height,
        allow: el.value().attr("allow").map(|s| s.to_string()),
        sandbox: el.value().attr("sandbox").map(|s| s.to_string()),
        loading: el.value().attr("loading").map(|s| s.to_string()),
        frameborder: el.value().attr("frameborder").map(|s| s.to_string()),
    })
}

/// 提取 object 元素
fn extract_object(el: &ElementRef, base_url: Option<&Url>) -> Option<EmbeddedMedia> {
    let data = el.value().attr("data")?;

    if data.to_lowercase().contains(".pdf") {
        return None;
    }

    let platform = detect_embed_platform(data);

    if is_video_or_audio_platform(&platform) {
        return None;
    }

    Some(EmbeddedMedia {
        url: data.to_string(),
        absolute_url: resolve_url(data, base_url),
        platform,
        title: el.value().attr("title").map(|s| s.to_string()),
        width: el.value().attr("width").and_then(parse_dimension),
        height: el.value().attr("height").and_then(parse_dimension),
        ..Default::default()
    })
}

/// 提取 embed 元素
fn extract_embed_tag(el: &ElementRef, base_url: Option<&Url>) -> Option<EmbeddedMedia> {
    let src = el.value().attr("src")?;

    if src.to_lowercase().contains(".pdf") {
        return None;
    }

    let platform = detect_embed_platform(src);

    if is_video_or_audio_platform(&platform) {
        return None;
    }

    Some(EmbeddedMedia {
        url: src.to_string(),
        absolute_url: resolve_url(src, base_url),
        platform,
        title: None,
        width: el.value().attr("width").and_then(parse_dimension),
        height: el.value().attr("height").and_then(parse_dimension),
        ..Default::default()
    })
}

/// 提取社交嵌入
fn extract_social_embeds(
    document: &Html,
    embeds: &mut Vec<EmbeddedMedia>,
    seen_urls: &mut HashSet<String>,
) {
    if let Ok(sel) = Selector::parse("blockquote.twitter-tweet") {
        for el in document.select(&sel) {
            if let Ok(link_sel) = Selector::parse("a") {
                for link in el.select(&link_sel) {
                    if let Some(href) = link.value().attr("href") {
                        if seen_urls.insert(href.to_string()) {
                            embeds.push(EmbeddedMedia {
                                url: href.to_string(),
                                absolute_url: Some(href.to_string()),
                                platform: EmbedPlatform::Twitter,
                                ..Default::default()
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("blockquote.instagram-media") {
        for el in document.select(&sel) {
            if let Some(permalink) = el.value().attr("data-instgrm-permalink") {
                if seen_urls.insert(permalink.to_string()) {
                    embeds.push(EmbeddedMedia {
                        url: permalink.to_string(),
                        absolute_url: Some(permalink.to_string()),
                        platform: EmbedPlatform::Instagram,
                        ..Default::default()
                    });
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("div.fb-post, div.fb-video") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("data-href") {
                if seen_urls.insert(href.to_string()) {
                    embeds.push(EmbeddedMedia {
                        url: href.to_string(),
                        absolute_url: Some(href.to_string()),
                        platform: EmbedPlatform::Facebook,
                        ..Default::default()
                    });
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("blockquote.reddit-embed-bq") {
        for el in document.select(&sel) {
            if let Ok(link_sel) = Selector::parse("a") {
                for link in el.select(&link_sel) {
                    if let Some(href) = link.value().attr("href") {
                        if href.to_lowercase().contains("reddit.com") && seen_urls.insert(href.to_string()) {
                            embeds.push(EmbeddedMedia {
                                url: href.to_string(),
                                absolute_url: Some(href.to_string()),
                                platform: EmbedPlatform::Reddit,
                                ..Default::default()
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// 检测嵌入平台
pub fn detect_embed_platform(url: &str) -> EmbedPlatform {
    if GOOGLE_MAPS.is_match(url) { return EmbedPlatform::GoogleMaps; }
    if TWITTER.is_match(url) { return EmbedPlatform::Twitter; }
    if INSTAGRAM.is_match(url) { return EmbedPlatform::Instagram; }
    if FACEBOOK.is_match(url) { return EmbedPlatform::Facebook; }
    if LINKEDIN.is_match(url) { return EmbedPlatform::LinkedIn; }
    if PINTEREST.is_match(url) { return EmbedPlatform::Pinterest; }
    if TIKTOK.is_match(url) { return EmbedPlatform::TikTok; }
    if REDDIT.is_match(url) { return EmbedPlatform::Reddit; }
    if CODEPEN.is_match(url) { return EmbedPlatform::CodePen; }
    if JSFIDDLE.is_match(url) { return EmbedPlatform::JsFiddle; }
    if CODESANDBOX.is_match(url) { return EmbedPlatform::CodeSandbox; }
    if GIPHY.is_match(url) { return EmbedPlatform::Giphy; }
    if SLIDESHARE.is_match(url) { return EmbedPlatform::SlideShare; }
    if TYPEFORM.is_match(url) { return EmbedPlatform::Typeform; }
    if CALENDLY.is_match(url) { return EmbedPlatform::Calendly; }
    if STRIPE.is_match(url) { return EmbedPlatform::Stripe; }
    if PAYPAL.is_match(url) { return EmbedPlatform::PayPal; }

    EmbedPlatform::Other
}

/// 判断是否为视频/音频平台（由其他模块处理，此处跳过）
fn is_video_or_audio_platform(platform: &EmbedPlatform) -> bool {
    matches!(platform,
        EmbedPlatform::YouTube |
        EmbedPlatform::Vimeo |
        EmbedPlatform::Dailymotion |
        EmbedPlatform::Twitch |
        EmbedPlatform::Wistia |
        EmbedPlatform::Spotify |
        EmbedPlatform::SoundCloud |
        EmbedPlatform::ApplePodcasts
    )
}

/// 解析尺寸（处理 px、% 等后缀）
fn parse_dimension(s: &str) -> Option<u32> {
    s.trim()
        .trim_end_matches("px")
        .trim_end_matches('%')
        .parse()
        .ok()
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

/// 从 HTML 字符串提取嵌入内容
pub fn extract_embeds_from_html(html: &str, base_url: Option<&str>) -> MediaResult<Vec<EmbeddedMedia>> {
    let document = Html::parse_document(html);
    let base = base_url.and_then(|u| Url::parse(u).ok());
    Ok(extract_embeds(&document, base.as_ref()))
}

/// 获取所有嵌入 URL
pub fn get_embed_urls(html: &str, base_url: Option<&str>) -> Vec<String> {
    extract_embeds_from_html(html, base_url)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| e.absolute_url)
        .collect()
}

/// 检查 HTML 是否包含嵌入内容
pub fn has_embeds(document: &Html) -> bool {
    if let Ok(sel) = Selector::parse("iframe[src], object[data], embed[src]") {
        document.select(&sel).any(|el| !is_within_noscript(&el))
    } else {
        false
    }
}

/// 按平台过滤嵌入
pub fn filter_by_platform(embeds: &[EmbeddedMedia], platform: EmbedPlatform) -> Vec<&EmbeddedMedia> {
    embeds.iter()
        .filter(|e| e.platform == platform)
        .collect()
}

/// 获取地图嵌入
pub fn get_maps(embeds: &[EmbeddedMedia]) -> Vec<&EmbeddedMedia> {
    filter_by_platform(embeds, EmbedPlatform::GoogleMaps)
}

/// 获取社交嵌入
pub fn get_social_embeds(embeds: &[EmbeddedMedia]) -> Vec<&EmbeddedMedia> {
    embeds.iter()
        .filter(|e| matches!(e.platform,
            EmbedPlatform::Twitter |
            EmbedPlatform::Instagram |
            EmbedPlatform::Facebook |
            EmbedPlatform::LinkedIn |
            EmbedPlatform::Pinterest |
            EmbedPlatform::TikTok |
            EmbedPlatform::Reddit
        ))
        .collect()
}

/// 获取代码嵌入
pub fn get_code_embeds(embeds: &[EmbeddedMedia]) -> Vec<&EmbeddedMedia> {
    embeds.iter()
        .filter(|e| matches!(e.platform,
            EmbedPlatform::CodePen |
            EmbedPlatform::JsFiddle |
            EmbedPlatform::CodeSandbox
        ))
        .collect()
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
    fn test_extract_google_maps_iframe() {
        let html = r#"<iframe src="https://www.google.com/maps/embed?pb=..." width="600" height="450"></iframe>"#;
        let doc = parse_html(html);
        let embeds = extract_embeds(&doc, None);

        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0].platform, EmbedPlatform::GoogleMaps);
        assert_eq!(embeds[0].width, Some(600));
        assert_eq!(embeds[0].height, Some(450));
    }

    #[test]
    fn test_detect_platform() {
        assert_eq!(detect_embed_platform("https://www.google.com/maps/embed"), EmbedPlatform::GoogleMaps);
        assert_eq!(detect_embed_platform("https://twitter.com/user/status/123"), EmbedPlatform::Twitter);
        assert_eq!(detect_embed_platform("https://codepen.io/user/pen/abc"), EmbedPlatform::CodePen);
        assert_eq!(detect_embed_platform("https://example.com/widget"), EmbedPlatform::Other);
    }

    #[test]
    fn test_has_embeds() {
        let with_embed = r#"<iframe src="https://example.com"></iframe>"#;
        let without_embed = r#"<div>No embed</div>"#;

        assert!(has_embeds(&parse_html(with_embed)));
        assert!(!has_embeds(&parse_html(without_embed)));
    }

    #[test]
    fn test_skip_noscript_iframe() {
        let html = r#"
            <iframe src="https://example.com/visible"></iframe>
            <noscript><iframe src="https://example.com/hidden"></iframe></noscript>
        "#;
        let doc = parse_html(html);
        let embeds = extract_embeds(&doc, None);

        // 如果 scraper 把 noscript 内容作为文本解析，这里只有1个 iframe
        // 如果作为元素解析，is_within_noscript 会过滤掉 hidden
        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0].url, "https://example.com/visible");
    }

    #[test]
    fn test_noscript_only_iframe() {
        let html = r#"<noscript><iframe src="https://example.com/hidden"></iframe></noscript>"#;
        let doc = parse_html(html);
        let embeds = extract_embeds(&doc, None);

        // 验证 scraper 是否把 noscript 内部的 iframe 解析为 DOM 元素
        let iframe_sel = Selector::parse("iframe[src]").unwrap();
        let dom_count = doc.select(&iframe_sel).count();
        eprintln!("noscript 内 iframe DOM 元素数量: {}", dom_count);

        // 无论 scraper 如何解析 noscript，都不应提取
        assert!(embeds.is_empty());
    }

    #[test]
    fn test_skip_noscript_object() {
        let html = r#"
            <object data="https://example.com/widget"></object>
            <noscript><object data="https://example.com/noscript-widget"></object></noscript>
        "#;
        let doc = parse_html(html);
        let embeds = extract_embeds(&doc, None);

        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0].url, "https://example.com/widget");
    }

    #[test]
    fn test_skip_noscript_embed() {
        let html = r#"
            <embed src="https://example.com/plugin">
            <noscript><embed src="https://example.com/noscript-plugin"></noscript>
        "#;
        let doc = parse_html(html);
        let embeds = extract_embeds(&doc, None);

        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0].url, "https://example.com/plugin");
    }

    #[test]
    fn test_noscript_has_embeds() {
        let html = r#"<noscript><iframe src="https://example.com"></iframe></noscript>"#;
        assert!(!has_embeds(&parse_html(html)));
    }
}
