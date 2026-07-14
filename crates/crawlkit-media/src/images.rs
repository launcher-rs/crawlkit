//! 图片提取
//!
//! 支持：
//! - `<img>` 标签（src、data-src 等懒加载属性）
//! - `<picture>` 元素（<source> 子标签）
//! - srcset 响应式图片解析
//! - 占位图检测
//! - 装饰性图片识别

use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector, ElementRef};
use std::collections::HashSet;
use url::Url;

use crate::types::{
    ImageMedia, ImageFormat, ImageLoading, SrcsetEntry, MediaResult,
};

lazy_static! {
    /// 常见的懒加载 data 属性
    static ref LAZY_ATTRS: Vec<&'static str> = vec![
        "data-src",
        "data-lazy-src",
        "data-original",
        "data-srcset",
        "data-lazy-srcset",
        "data-bg",
        "data-background",
        "data-image",
        "data-url",
    ];

    /// 占位图 URL 模式
    #[allow(clippy::unwrap_used)]
    static ref PLACEHOLDER_PATTERN: Regex = Regex::new(
        r"(?i)(placeholder|blank|spacer|pixel|1x1|loading|lazy)"
    ).unwrap();

    /// data URL 模式
    #[allow(clippy::unwrap_used)]
    static ref DATA_URL: Regex = Regex::new(r"^data:image/").unwrap();
}

/// 从 HTML 文档提取所有图片
pub fn extract_images(document: &Html, base_url: Option<&Url>) -> Vec<ImageMedia> {
    let mut images = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    if let Ok(sel) = Selector::parse("img") {
        for el in document.select(&sel) {
            if let Some(img) = extract_image_element(&el, base_url) {
                let key = img.absolute_url.as_ref().unwrap_or(&img.src).clone();
                if seen_urls.insert(key) {
                    images.push(img);
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("picture") {
        for el in document.select(&sel) {
            for img in extract_picture_element(&el, base_url) {
                let key = img.absolute_url.as_ref().unwrap_or(&img.src).clone();
                if seen_urls.insert(key) {
                    images.push(img);
                }
            }
        }
    }

    images
}

/// 提取单个图片元素
fn extract_image_element(el: &ElementRef, base_url: Option<&Url>) -> Option<ImageMedia> {
    let src = get_image_src(el)?;

    if DATA_URL.is_match(&src) {
        return None;
    }

    let absolute_url = resolve_url(&src, base_url);

    let format = absolute_url.as_ref()
        .and_then(|u| extract_extension(u))
        .or_else(|| extract_extension(&src))
        .map_or(ImageFormat::Unknown, |ext| ImageFormat::from_extension(&ext));

    let width = el.value().attr("width")
        .and_then(|w| w.trim_end_matches("px").parse().ok());
    let height = el.value().attr("height")
        .and_then(|h| h.trim_end_matches("px").parse().ok());

    let alt = el.value().attr("alt").map(std::string::ToString::to_string);
    let is_decorative = alt.as_ref().is_some_and(std::string::String::is_empty);

    let loading = match el.value().attr("loading") {
        Some("lazy") => ImageLoading::Lazy,
        _ => ImageLoading::Eager,
    };

    let srcset = el.value().attr("srcset")
        .map(|s| parse_srcset(s, base_url))
        .unwrap_or_default();

    let data_src = LAZY_ATTRS.iter()
        .find_map(|attr| el.value().attr(attr))
        .map(std::string::ToString::to_string);

    let is_placeholder = is_placeholder_image(&src, width, height);

    let classes: Vec<String> = el.value().classes().map(std::string::ToString::to_string).collect();
    let id = el.value().attr("id").map(std::string::ToString::to_string);

    let mime_type = format.mime_type();

    Some(ImageMedia {
        src,
        absolute_url,
        alt,
        title: el.value().attr("title").map(std::string::ToString::to_string),
        width,
        height,
        format,
        mime_type: Some(mime_type.to_string()),
        loading,
        is_decorative,
        srcset,
        sizes: el.value().attr("sizes").map(std::string::ToString::to_string),
        data_src,
        is_placeholder,
        size_bytes: None,
        content_hash: None,
        classes,
        id,
    })
}

/// 提取 picture 元素中的图片
fn extract_picture_element(picture: &ElementRef, base_url: Option<&Url>) -> Vec<ImageMedia> {
    let mut images = Vec::new();

    if let Ok(img_sel) = Selector::parse("img")
        && let Some(img_el) = picture.select(&img_sel).next()
            && let Some(mut img) = extract_image_element(&img_el, base_url) {
                if let Ok(source_sel) = Selector::parse("source") {
                    for source in picture.select(&source_sel) {
                        if let Some(srcset_str) = source.value().attr("srcset") {
                            let source_entries = parse_srcset(srcset_str, base_url);
                            img.srcset.extend(source_entries);
                        }
                    }
                }
                images.push(img);
            }

    images
}

/// 从多种属性中获取图片 URL
fn get_image_src(el: &ElementRef) -> Option<String> {
    el.value().attr("src")
        .filter(|s| !s.is_empty() && (!s.starts_with("data:image/svg+xml") || s.len() > 100))
        .or_else(|| {
            LAZY_ATTRS.iter()
                .find_map(|attr| el.value().attr(attr))
        })
        .map(std::string::ToString::to_string)
}

/// 解析 srcset 属性
fn parse_srcset(srcset: &str, base_url: Option<&Url>) -> Vec<SrcsetEntry> {
    let mut entries = Vec::new();

    for part in srcset.split(',') {
        let part = part.trim();
        let parts: Vec<&str> = part.split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        let url = parts[0].to_string();
        let resolved_url = resolve_url(&url, base_url).unwrap_or(url);

        let mut width = None;
        let mut density = None;

        if parts.len() > 1 {
            let descriptor = parts[1];
            if descriptor.ends_with('w') {
                width = descriptor.trim_end_matches('w').parse().ok();
            } else if descriptor.ends_with('x') {
                density = descriptor.trim_end_matches('x').parse().ok();
            }
        }

        entries.push(SrcsetEntry {
            url: resolved_url,
            width,
            density,
        });
    }

    entries
}

/// 检查是否为占位图
fn is_placeholder_image(src: &str, width: Option<u32>, height: Option<u32>) -> bool {
    if PLACEHOLDER_PATTERN.is_match(src) {
        return true;
    }

    if let (Some(w), Some(h)) = (width, height)
        && w <= 10 && h <= 10 {
            return true;
        }

    let placeholders = [
        "placehold.it",
        "placeholder.com",
        "placekitten.com",
        "picsum.photos",
        "via.placeholder.com",
    ];

    placeholders.iter().any(|p| src.contains(p))
}

/// 从 URL 提取文件扩展名
fn extract_extension(url: &str) -> Option<String> {
    let path = url.split('?').next()?.split('#').next()?;
    let filename = path.rsplit('/').next()?;

    if !filename.contains('.') {
        return None;
    }

    let ext = filename.rsplit('.').next()?;

    if ext != filename && ext.len() <= 5 && ext.chars().all(char::is_alphanumeric) {
        Some(ext.to_lowercase())
    } else {
        None
    }
}

/// 解析相对 URL 为绝对 URL
fn resolve_url(href: &str, base_url: Option<&Url>) -> Option<String> {
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }

    if href.starts_with("//") {
        return Some(format!("https:{href}"));
    }

    if href.starts_with("data:") {
        return Some(href.to_string());
    }

    base_url.and_then(|base| base.join(href).ok().map(|u| u.to_string()))
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 从 HTML 字符串提取图片
pub fn extract_images_from_html(html: &str, base_url: Option<&str>) -> MediaResult<Vec<ImageMedia>> {
    let document = Html::parse_document(html);
    let base = base_url.and_then(|u| Url::parse(u).ok());
    Ok(extract_images(&document, base.as_ref()))
}

/// 获取所有图片 URL
pub fn get_image_urls(html: &str, base_url: Option<&str>) -> Vec<String> {
    extract_images_from_html(html, base_url)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|img| img.absolute_url)
        .collect()
}

/// 检查 HTML 是否包含图片
pub fn has_images(document: &Html) -> bool {
    if let Ok(sel) = Selector::parse("img, picture") {
        document.select(&sel).next().is_some()
    } else {
        false
    }
}

/// 获取分辨率最高的图片 URL
pub fn get_best_image_url(img: &ImageMedia) -> &str {
    if !img.srcset.is_empty() {
        if let Some(entry) = img.srcset.iter()
            .filter(|e| e.width.is_some())
            .max_by_key(|e| e.width)
        {
            return &entry.url;
        }

        if let Some(entry) = img.srcset.iter()
            .filter(|e| e.density.is_some())
            .max_by(|a, b| a.density.partial_cmp(&b.density).unwrap_or(std::cmp::Ordering::Equal))
        {
            return &entry.url;
        }
    }

    img.absolute_url.as_deref().unwrap_or(&img.src)
}

/// 过滤占位图
pub fn filter_placeholders(images: Vec<ImageMedia>) -> Vec<ImageMedia> {
    images.into_iter()
        .filter(|img| !img.is_placeholder)
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
    fn test_extract_basic_image() {
        let html = r#"<html><body><img src="/images/test.jpg" alt="Test image"></body></html>"#;
        let doc = parse_html(html);
        let base = Url::parse("https://example.com").unwrap();
        let images = extract_images(&doc, Some(&base));

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].src, "/images/test.jpg");
        assert_eq!(images[0].absolute_url, Some("https://example.com/images/test.jpg".to_string()));
        assert_eq!(images[0].alt, Some("Test image".to_string()));
    }

    #[test]
    fn test_extract_lazy_loaded_image() {
        let html = r#"<img src="placeholder.gif" data-src="/real-image.jpg" loading="lazy">"#;
        let doc = parse_html(html);
        let images = extract_images(&doc, None);

        assert!(!images.is_empty());
        assert_eq!(images[0].loading, ImageLoading::Lazy);
        assert!(images[0].data_src.is_some());
    }

    #[test]
    fn test_has_images() {
        let html_with = "<html><body><img src='test.jpg'></body></html>";
        let html_without = "<html><body><p>No images</p></body></html>";

        assert!(has_images(&parse_html(html_with)));
        assert!(!has_images(&parse_html(html_without)));
    }

    #[test]
    fn test_filter_placeholders() {
        let images = vec![
            ImageMedia { src: "real.jpg".to_string(), is_placeholder: false, ..Default::default() },
            ImageMedia { src: "placeholder.png".to_string(), is_placeholder: true, ..Default::default() },
        ];

        let filtered = filter_placeholders(images);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].src, "real.jpg");
    }
}
