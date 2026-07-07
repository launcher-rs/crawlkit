//! 页面元数据提取模块
//!
//! 从 HTML 文档中提取标题、描述、关键词、Open Graph、Twitter Card、
//! 结构化数据等各类元数据。改编自 halldyll-parser。

use scraper::{Html, ElementRef};
use std::collections::HashMap;
use url::Url;

use crate::selector::{
    SELECTORS, try_parse_selector, meta_name_selector, meta_property_selector, link_rel_selector,
};
use crate::types::{
    PageMetadata, OpenGraph, TwitterCard, RobotsMeta, AlternateLink,
    StructuredData, ParserResult,
};

// ============================================================================
// 主入口
// ============================================================================

/// 从 HTML 文档中提取完整页面元数据
pub fn extract_metadata(document: &Html, base_url: Option<&Url>) -> ParserResult<PageMetadata> {
    let title = extract_title(document);
    let charset = extract_charset(document);
    let language = extract_language(document);
    let base = extract_base_url(document);
    let description = extract_meta_content(document, "description");
    let author = extract_meta_content(document, "author");
    let generator = extract_meta_content(document, "generator");
    let viewport = extract_meta_content(document, "viewport");
    let theme_color = extract_meta_content(document, "theme-color");
    let published_date = extract_meta_content(document, "date")
        .or_else(|| extract_meta_content(document, "article:published_time"));
    let modified_date = extract_meta_content(document, "article:modified_time");
    let keywords = extract_keywords(document);
    let canonical = extract_canonical(document, base_url);
    let favicon = extract_favicon(document, base_url);
    let apple_touch = extract_apple_touch_icon(document, base_url);
    let robots = extract_robots(document);
    let opengraph = extract_opengraph(document);
    let twitter = extract_twitter_card(document);
    let alternates = extract_alternates(document, base_url);
    let structured = extract_structured_data(document);
    let schema_type = structured.first()
        .and_then(|s| s.schema_type.clone());
    let custom = extract_custom_meta(document);

    Ok(PageMetadata {
        title,
        description,
        keywords,
        author,
        generator,
        canonical,
        base_url: base,
        language,
        charset,
        viewport,
        robots,
        opengraph,
        twitter,
        alternates,
        favicon,
        apple_touch_icon: apple_touch,
        theme_color,
        published_date,
        modified_date,
        schema_type,
        custom,
    })
}

// ============================================================================
// 基本元数据
// ============================================================================

/// 提取页面标题：依次尝试 og:title、twitter:title、`<title>`、h1
pub fn extract_title(document: &Html) -> Option<String> {
    if let Some(sel) = try_parse_selector(&meta_property_selector("og:title")) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let text = content.trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    if let Some(sel) = try_parse_selector(&meta_name_selector("twitter:title")) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let text = content.trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    let title_sel = &SELECTORS.title;
    if let Some(el) = document.select(title_sel).next() {
        let text: String = el.text().collect::<Vec<_>>().join("").trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    if let Some(sel) = try_parse_selector("h1") {
        if let Some(el) = document.select(&sel).next() {
            let text: String = el.text().collect::<Vec<_>>().join("").trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// 提取字符编码声明
pub fn extract_charset(document: &Html) -> Option<String> {
    if let Some(sel) = try_parse_selector("meta[charset]") {
        if let Some(el) = document.select(&sel).next() {
            if let Some(charset) = el.value().attr("charset") {
                let val = charset.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    if let Some(sel) = try_parse_selector(r#"meta[http-equiv="Content-Type"]"#) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                if let Some(pos) = content.to_lowercase().find("charset=") {
                    let charset = content[pos + 8..].trim().to_string();
                    if !charset.is_empty() {
                        return Some(charset);
                    }
                }
            }
        }
    }
    None
}

/// 提取页面语言：优先 `<html lang>`，其次 `http-equiv="content-language"`
pub fn extract_language(document: &Html) -> Option<String> {
    let html_sel = &SELECTORS.html;
    if let Some(el) = document.select(html_sel).next() {
        if let Some(lang) = el.value().attr("lang") {
            let val = lang.trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    if let Some(sel) = try_parse_selector(r#"meta[http-equiv="content-language"]"#) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let val = content.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// 提取 `<base>` 标签的 href 值
pub fn extract_base_url(document: &Html) -> Option<String> {
    let base_sel = &SELECTORS.base;
    if let Some(el) = document.select(base_sel).next() {
        if let Some(href) = el.value().attr("href") {
            let val = href.trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

/// 通用 meta 内容提取，同时尝试 `name` 和 `property` 属性
pub fn extract_meta_content(document: &Html, name: &str) -> Option<String> {
    if let Some(sel) = try_parse_selector(&meta_name_selector(name)) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let val = content.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    if let Some(sel) = try_parse_selector(&meta_property_selector(name)) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let val = content.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// 提取关键词：从 `<meta name="keywords">` 解析，逗号分隔
pub fn extract_keywords(document: &Html) -> Vec<String> {
    if let Some(sel) = try_parse_selector(&meta_name_selector("keywords")) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                return content
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}

// ============================================================================
// 链接资源
// ============================================================================

/// 提取规范链接 `<link rel="canonical">`
pub fn extract_canonical(document: &Html, base_url: Option<&Url>) -> Option<String> {
    if let Some(sel) = try_parse_selector(&link_rel_selector("canonical")) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                return resolve_url(base_url, href);
            }
        }
    }
    None
}

/// 提取 favicon：优先选择尺寸最大的图标
pub fn extract_favicon(document: &Html, base_url: Option<&Url>) -> Option<String> {
    if let Some(sel) = try_parse_selector("link[rel~='icon'], link[rel='shortcut icon'], link[rel='apple-touch-icon-precomposed']") {
        let mut candidates: Vec<(String, String)> = Vec::new();
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let sizes = el.value().attr("sizes").unwrap_or("").to_string();
                candidates.push((href.to_string(), sizes));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let best = find_largest_icon(&candidates);
        resolve_url(base_url, &best)
    } else {
        None
    }
}

/// 提取苹果触控图标
pub fn extract_apple_touch_icon(document: &Html, base_url: Option<&Url>) -> Option<String> {
    if let Some(sel) = try_parse_selector("link[rel='apple-touch-icon'], link[rel='apple-touch-icon-precomposed']") {
        if let Some(el) = document.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                return resolve_url(base_url, href);
            }
        }
    }
    None
}

// ============================================================================
// 爬虫规则（robots）
// ============================================================================

/// 提取 robots meta 标签，解析指令
pub fn extract_robots(document: &Html) -> RobotsMeta {
    let content = if let Some(sel) = try_parse_selector("meta[name='robots'], meta[name='ROBOTS']") {
        document.select(&sel).next()
            .and_then(|el| el.value().attr("content").map(|c| c.to_string()))
    } else {
        None
    };

    let directives = match content {
        Some(ref c) => c.split(',').map(|s| s.trim().to_lowercase()).collect::<Vec<_>>(),
        None => return RobotsMeta::allowed(),
    };

    let mut meta = RobotsMeta::allowed();
    meta.raw = content;

    for directive in &directives {
        match directive.as_str() {
            "noindex" => meta.index = false,
            "nofollow" => meta.follow = false,
            "noarchive" => meta.archive = false,
            "nocache" => meta.cache = false,
            "nosnippet" => meta.snippet = false,
            "all" | "index" | "follow" => {}
            v if v.starts_with("max-snippet:") => {
                let val = v.trim_start_matches("max-snippet:").trim();
                meta.max_snippet = val.parse().unwrap_or(-1);
            }
            v if v.starts_with("max-image-preview:") => {
                let val = v.trim_start_matches("max-image-preview:").trim();
                meta.max_image_preview = Some(val.to_string());
            }
            v if v.starts_with("max-video-preview:") => {
                let val = v.trim_start_matches("max-video-preview:").trim();
                meta.max_video_preview = val.parse().unwrap_or(-1);
            }
            _ => {}
        }
    }

    meta
}

// ============================================================================
// Open Graph
// ============================================================================

/// 提取 Open Graph 元数据
pub fn extract_opengraph(document: &Html) -> OpenGraph {
    let mut og = OpenGraph::default();
    if let Some(title) = extract_og_property(document, "og:title") {
        og.title = Some(title);
    }
    if let Some(og_type) = extract_og_property(document, "og:type") {
        og.og_type = Some(og_type);
    }
    if let Some(url) = extract_og_property(document, "og:url") {
        og.url = Some(url);
    }
    if let Some(image) = extract_og_property(document, "og:image") {
        og.image = Some(image);
    }
    if let Some(desc) = extract_og_property(document, "og:description") {
        og.description = Some(desc);
    }
    if let Some(site) = extract_og_property(document, "og:site_name") {
        og.site_name = Some(site);
    }
    if let Some(locale) = extract_og_property(document, "og:locale") {
        og.locale = Some(locale);
    }
    if let Some(video) = extract_og_property(document, "og:video") {
        og.video = Some(video);
    }
    if let Some(audio) = extract_og_property(document, "og:audio") {
        og.audio = Some(audio);
    }
    og.extra = extract_all_og_properties(document);
    og
}

/// 提取单个 Open Graph 属性值
pub fn extract_og_property(document: &Html, property: &str) -> Option<String> {
    if let Some(sel) = try_parse_selector(&meta_property_selector(property)) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let val = content.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// 提取所有 Open Graph 属性到 HashMap（不含标准字段）
pub fn extract_all_og_properties(document: &Html) -> HashMap<String, String> {
    let mut extras = HashMap::new();
    let meta_sel = &SELECTORS.meta;
    let known = [
        "og:title", "og:type", "og:url", "og:image", "og:description",
        "og:site_name", "og:locale", "og:video", "og:audio",
    ];
    for el in document.select(meta_sel) {
        if let Some(property) = el.value().attr("property") {
            let prop_lower = property.to_lowercase();
            if prop_lower.starts_with("og:") && !known.contains(&prop_lower.as_str()) {
                if let Some(content) = el.value().attr("content") {
                    extras.insert(property.to_string(), content.to_string());
                }
            }
        }
    }
    extras
}

// ============================================================================
// Twitter Card
// ============================================================================

/// 提取 Twitter Card 元数据
pub fn extract_twitter_card(document: &Html) -> TwitterCard {
    let mut card = TwitterCard::default();
    if let Some(val) = extract_twitter_property(document, "twitter:card") {
        card.card = Some(val);
    }
    if let Some(val) = extract_twitter_property(document, "twitter:site") {
        card.site = Some(val);
    }
    if let Some(val) = extract_twitter_property(document, "twitter:creator") {
        card.creator = Some(val);
    }
    if let Some(val) = extract_twitter_property(document, "twitter:title") {
        card.title = Some(val);
    }
    if let Some(val) = extract_twitter_property(document, "twitter:description") {
        card.description = Some(val);
    }
    if let Some(val) = extract_twitter_property(document, "twitter:image") {
        card.image = Some(val);
    }
    card.extra = extract_all_twitter_properties(document);
    card
}

/// 提取单个 Twitter Card 属性值
pub fn extract_twitter_property(document: &Html, name: &str) -> Option<String> {
    if let Some(sel) = try_parse_selector(&meta_name_selector(name)) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let val = content.trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// 提取所有 Twitter Card 属性到 HashMap（不含标准字段）
pub fn extract_all_twitter_properties(document: &Html) -> HashMap<String, String> {
    let mut extras = HashMap::new();
    let meta_sel = &SELECTORS.meta;
    let known = [
        "twitter:card", "twitter:site", "twitter:creator",
        "twitter:title", "twitter:description", "twitter:image",
    ];
    for el in document.select(meta_sel) {
        if let Some(name) = el.value().attr("name") {
            let name_lower = name.to_lowercase();
            if name_lower.starts_with("twitter:") && !known.contains(&name_lower.as_str()) {
                if let Some(content) = el.value().attr("content") {
                    extras.insert(name.to_string(), content.to_string());
                }
            }
        }
    }
    extras
}

// ============================================================================
// 备用链接
// ============================================================================

/// 提取 `<link rel="alternate">` 标签（如多语言版本、RSS 等）
pub fn extract_alternates(document: &Html, base_url: Option<&Url>) -> Vec<AlternateLink> {
    let mut alternates = Vec::new();
    if let Some(sel) = try_parse_selector(&link_rel_selector("alternate")) {
        for el in document.select(&sel) {
            let hreflang = el.value().attr("hreflang")
                .unwrap_or("")
                .trim()
                .to_string();
            let href = el.value().attr("href")
                .unwrap_or("")
                .trim()
                .to_string();
            if href.is_empty() {
                continue;
            }
            let resolved = resolve_url(base_url, &href).unwrap_or(href);
            alternates.push(AlternateLink { hreflang, href: resolved });
        }
    }
    alternates
}

// ============================================================================
// 结构化数据
// ============================================================================

/// 提取全部结构化数据（JSON-LD + Microdata）
pub fn extract_structured_data(document: &Html) -> Vec<StructuredData> {
    let mut results = Vec::new();
    results.extend(extract_json_ld(document));
    results.extend(extract_microdata(document));
    results
}

/// 提取 JSON-LD 结构化数据
pub fn extract_json_ld(document: &Html) -> Vec<StructuredData> {
    let mut results = Vec::new();
    let json_ld_sel = &SELECTORS.json_ld;
    for el in document.select(json_ld_sel) {
        let raw = el.text().collect::<Vec<_>>().join("").trim().to_string();
        if raw.is_empty() {
            continue;
        }
        let mut sd = StructuredData::json_ld(&raw);
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(obj) = val.as_object() {
                for (k, v) in obj {
                    sd.properties.insert(k.clone(), v.clone());
                }
            }
            if let Some(typ) = val.get("@type").and_then(|t| t.as_str()) {
                sd.schema_type = Some(typ.to_string());
            }
            if let Some(typ) = val.get("type").and_then(|t| t.as_str()) {
                sd.schema_type = Some(typ.to_string());
            }
        }
        results.push(sd);
    }
    results
}

/// 提取 Microdata 结构化数据
pub fn extract_microdata(document: &Html) -> Vec<StructuredData> {
    let mut results = Vec::new();
    if let Some(sel) = try_parse_selector("*[itemscope]") {
        for el in document.select(&sel) {
            let itemtype = el.value().attr("itemtype")
                .and_then(|t| t.rsplit('/').next())
                .or_else(|| el.value().attr("itemtype"))
                .map(|t| t.to_string());

            let mut sd = match itemtype {
                Some(ref t) => StructuredData::microdata(t),
                None => continue,
            };

            let mut props = HashMap::new();
            extract_microdata_properties(&el, &mut props);
            sd.properties = props;
            results.push(sd);
        }
    }
    results
}

/// 递归提取一个 Microdata 元素的所有属性
fn extract_microdata_properties(element: &ElementRef, props: &mut HashMap<String, serde_json::Value>) {
    if let Some(sel) = try_parse_selector("*[itemprop]") {
        for child in element.select(&sel) {
            if child.value().attr("itemscope").is_some() {
                continue;
            }
            if let Some(prop_name) = child.value().attr("itemprop") {
                let value = get_microdata_value(&child);
                props.insert(prop_name.to_string(), serde_json::Value::String(value));
            }
        }
    }
}

/// 获取 Microdata 元素的值（按优先级：content → src → href → text）
fn get_microdata_value(element: &ElementRef) -> String {
    if let Some(content) = element.value().attr("content") {
        return content.trim().to_string();
    }
    if let Some(src) = element.value().attr("src") {
        return src.trim().to_string();
    }
    if let Some(href) = element.value().attr("href") {
        return href.trim().to_string();
    }
    element.text().collect::<Vec<_>>().join("").trim().to_string()
}

// ============================================================================
// 自定义元数据
// ============================================================================

/// 提取所有未归类到其他方法的自定义 meta 标签
pub fn extract_custom_meta(document: &Html) -> HashMap<String, String> {
    let mut custom = HashMap::new();
    let meta_sel = &SELECTORS.meta;
    let skip_names = [
        "description", "keywords", "author", "generator", "viewport",
        "theme-color", "robots", "googlebot", "date",
    ];
    let skip_prefixes = ["og:", "twitter:", "article:", "al:"];
    for el in document.select(meta_sel) {
        let name = el.value().attr("name")
            .or_else(|| el.value().attr("property"))
            .map(|n| n.to_string());
        let content = el.value().attr("content")
            .map(|c| c.to_string());
        match (name, content) {
            (Some(n), Some(c)) if !n.is_empty() && !c.is_empty() => {
                let n_lower = n.to_lowercase();
                if skip_names.contains(&n_lower.as_str()) {
                    continue;
                }
                if skip_prefixes.iter().any(|p| n_lower.starts_with(p)) {
                    continue;
                }
                custom.insert(n, c);
            }
            _ => {}
        }
    }
    custom
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 将相对 URL 解析为绝对 URL，base_url 为 None 时直接返回原始值
fn resolve_url(base_url: Option<&Url>, url_str: &str) -> Option<String> {
    let trimmed = url_str.trim();
    if trimmed.is_empty() || trimmed.starts_with("data:") || trimmed.starts_with("javascript:") {
        return None;
    }
    match base_url {
        Some(base) => base.join(trimmed).ok().map(|u| u.to_string()),
        None => Some(trimmed.to_string()),
    }
}

/// 从候选图标列表中找出尺寸最大的一个
fn find_largest_icon(candidates: &[(String, String)]) -> String {
    let mut best: Option<&str> = None;
    let mut best_size: i32 = -1;

    for (href, sizes) in candidates {
        if let Some(dim) = parse_icon_size(sizes) {
            if dim > best_size {
                best_size = dim;
                best = Some(href);
            }
        } else if best.is_none() {
            best = Some(href);
        }
    }

    best.unwrap_or_else(|| candidates[0].0.as_str()).to_string()
}

/// 解析 sizes 属性（如 "32x32"）返回最大边长
fn parse_icon_size(sizes: &str) -> Option<i32> {
    let sizes = sizes.trim();
    if sizes.is_empty() || sizes.eq_ignore_ascii_case("any") {
        return None;
    }
    sizes.split_whitespace()
        .filter_map(|s| {
            let parts: Vec<&str> = s.split('x').collect();
            if parts.len() == 2 {
                let w = parts[0].parse::<i32>().ok()?;
                let h = parts[1].parse::<i32>().ok()?;
                Some(w.max(h))
            } else {
                None
            }
        })
        .max()
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 解析 HTML 字符串为 `Html` 文档
    fn parse_html(html: &str) -> Html {
        Html::parse_document(html)
    }

    /// 创建测试用 base URL
    fn test_base_url() -> Url {
        Url::parse("https://example.com").unwrap()
    }

    // -----------------------------------------------------------------------
    // 标题提取
    // -----------------------------------------------------------------------

    #[test]
    fn 提取标题_og_title优先() {
        let html = parse_html(r#"
            <html><head>
                <meta property="og:title" content="OG Title">
                <title>Page Title</title>
            </head></html>
        "#);
        assert_eq!(extract_title(&html), Some("OG Title".to_string()));
    }

    #[test]
    fn 提取标题_twitter_title其次() {
        let html = parse_html(r#"
            <html><head>
                <meta name="twitter:title" content="Twitter Title">
                <title>Page Title</title>
            </head></html>
        "#);
        assert_eq!(extract_title(&html), Some("Twitter Title".to_string()));
    }

    #[test]
    fn 提取标题_title标签兜底() {
        let html = parse_html("<html><head><title>Page Title</title></head></html>");
        assert_eq!(extract_title(&html), Some("Page Title".to_string()));
    }

    #[test]
    fn 提取标题_h1最后兜底() {
        let html = parse_html("<html><body><h1>H1 Title</h1></body></html>");
        assert_eq!(extract_title(&html), Some("H1 Title".to_string()));
    }

    #[test]
    fn 提取标题_无标题返回None() {
        let html = parse_html("<html><head></head><body><p>no title</p></body></html>");
        assert_eq!(extract_title(&html), None);
    }

    // -----------------------------------------------------------------------
    // 字符编码
    // -----------------------------------------------------------------------

    #[test]
    fn 提取charset_meta标签() {
        let html = parse_html(r#"<html><head><meta charset="utf-8"></head></html>"#);
        assert_eq!(extract_charset(&html), Some("utf-8".to_string()));
    }

    #[test]
    fn 提取charset_http_equiv() {
        let html = parse_html(r#"<html><head><meta http-equiv="Content-Type" content="text/html; charset=gb2312"></head></html>"#);
        assert_eq!(extract_charset(&html), Some("gb2312".to_string()));
    }

    #[test]
    fn 提取charset_无声明返回None() {
        let html = parse_html("<html><head></head></html>");
        assert_eq!(extract_charset(&html), None);
    }

    // -----------------------------------------------------------------------
    // 语言
    // -----------------------------------------------------------------------

    #[test]
    fn 提取语言_html_lang属性() {
        let html = parse_html(r#"<html lang="en"><head></head></html>"#);
        assert_eq!(extract_language(&html), Some("en".to_string()));
    }

    #[test]
    fn 提取语言_http_equiv() {
        let html = parse_html(r#"<html><head><meta http-equiv="content-language" content="zh-CN"></head></html>"#);
        assert_eq!(extract_language(&html), Some("zh-CN".to_string()));
    }

    // -----------------------------------------------------------------------
    // base URL
    // -----------------------------------------------------------------------

    #[test]
    fn 提取base_url() {
        let html = parse_html(r#"<html><head><base href="https://example.com/base/"></head></html>"#);
        assert_eq!(extract_base_url(&html), Some("https://example.com/base/".to_string()));
    }

    // -----------------------------------------------------------------------
    // 关键词
    // -----------------------------------------------------------------------

    #[test]
    fn 提取关键词_逗号分隔() {
        let html = parse_html(r#"<html><head><meta name="keywords" content="rust, web, scraping"></head></html>"#);
        assert_eq!(extract_keywords(&html), vec!["rust", "web", "scraping"]);
    }

    #[test]
    fn 提取关键词_无关键词返回空() {
        let html = parse_html("<html><head></head></html>");
        assert!(extract_keywords(&html).is_empty());
    }

    // -----------------------------------------------------------------------
    // 规范链接
    // -----------------------------------------------------------------------

    #[test]
    fn 提取canonical_绝对路径() {
        let html = parse_html(r#"<html><head><link rel="canonical" href="https://example.com/page"></head></html>"#);
        assert_eq!(
            extract_canonical(&html, None),
            Some("https://example.com/page".to_string())
        );
    }

    #[test]
    fn 提取canonical_相对路径() {
        let html = parse_html(r#"<html><head><link rel="canonical" href="/blog/post"></head></html>"#);
        let base = test_base_url();
        assert_eq!(
            extract_canonical(&html, Some(&base)),
            Some("https://example.com/blog/post".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Favicon
    // -----------------------------------------------------------------------

    #[test]
    fn 提取favicon() {
        let html = parse_html(r#"<html><head><link rel="icon" href="/favicon.ico"></head></html>"#);
        let base = test_base_url();
        assert_eq!(
            extract_favicon(&html, Some(&base)),
            Some("https://example.com/favicon.ico".to_string())
        );
    }

    #[test]
    fn 提取favicon_选最大尺寸() {
        let html = parse_html(r#"
            <html><head>
                <link rel="icon" href="/small.png" sizes="16x16">
                <link rel="icon" href="/large.png" sizes="64x64">
            </head></html>
        "#);
        let base = test_base_url();
        assert_eq!(
            extract_favicon(&html, Some(&base)),
            Some("https://example.com/large.png".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Apple Touch Icon
    // -----------------------------------------------------------------------

    #[test]
    fn 提取apple_touch_icon() {
        let html = parse_html(r#"<html><head><link rel="apple-touch-icon" href="/apple-icon.png"></head></html>"#);
        let base = test_base_url();
        assert_eq!(
            extract_apple_touch_icon(&html, Some(&base)),
            Some("https://example.com/apple-icon.png".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Robots
    // -----------------------------------------------------------------------

    #[test]
    fn 提取robots_允许全部() {
        let html = parse_html(r#"<html><head><meta name="robots" content="all"></head></html>"#);
        let robots = extract_robots(&html);
        assert!(robots.index);
        assert!(robots.follow);
    }

    #[test]
    fn 提取robots_noindex_nofollow() {
        let html = parse_html(r#"<html><head><meta name="robots" content="noindex, nofollow"></head></html>"#);
        let robots = extract_robots(&html);
        assert!(!robots.index);
        assert!(!robots.follow);
    }

    #[test]
    fn 提取robots_无标签默认允许() {
        let html = parse_html("<html><head></head></html>");
        let robots = extract_robots(&html);
        assert!(robots.index);
        assert!(robots.follow);
    }

    #[test]
    fn 提取robots_max_snippet() {
        let html = parse_html(r#"<html><head><meta name="robots" content="max-snippet:50"></head></html>"#);
        let robots = extract_robots(&html);
        assert_eq!(robots.max_snippet, 50);
    }

    // -----------------------------------------------------------------------
    // Open Graph
    // -----------------------------------------------------------------------

    #[test]
    fn 提取opengraph_基本字段() {
        let html = parse_html(r#"
            <html><head>
                <meta property="og:title" content="OG Title">
                <meta property="og:description" content="OG Description">
                <meta property="og:url" content="https://example.com">
                <meta property="og:type" content="article">
            </head></html>
        "#);
        let og = extract_opengraph(&html);
        assert_eq!(og.title, Some("OG Title".to_string()));
        assert_eq!(og.description, Some("OG Description".to_string()));
        assert_eq!(og.url, Some("https://example.com".to_string()));
        assert_eq!(og.og_type, Some("article".to_string()));
        assert!(og.is_present());
    }

    #[test]
    fn 提取opengraph_扩展字段() {
        let html = parse_html(r#"
            <html><head>
                <meta property="og:title" content="Title">
                <meta property="og:custom" content="value">
            </head></html>
        "#);
        let og = extract_opengraph(&html);
        assert_eq!(og.extra.get("og:custom"), Some(&"value".to_string()));
    }

    // -----------------------------------------------------------------------
    // Twitter Card
    // -----------------------------------------------------------------------

    #[test]
    fn 提取twitter_card_基本字段() {
        let html = parse_html(r#"
            <html><head>
                <meta name="twitter:card" content="summary_large_image">
                <meta name="twitter:site" content="@example">
                <meta name="twitter:title" content="Tweet Title">
            </head></html>
        "#);
        let card = extract_twitter_card(&html);
        assert_eq!(card.card, Some("summary_large_image".to_string()));
        assert_eq!(card.site, Some("@example".to_string()));
        assert_eq!(card.title, Some("Tweet Title".to_string()));
    }

    // -----------------------------------------------------------------------
    // 备用链接
    // -----------------------------------------------------------------------

    #[test]
    fn 提取alternates() {
        let html = parse_html(r#"
            <html><head>
                <link rel="alternate" hreflang="zh" href="/zh/">
                <link rel="alternate" hreflang="en" href="/en/">
            </head></html>
        "#);
        let base = test_base_url();
        let alts = extract_alternates(&html, Some(&base));
        assert_eq!(alts.len(), 2);
        assert_eq!(alts[0].hreflang, "zh");
        assert_eq!(alts[0].href, "https://example.com/zh/");
        assert_eq!(alts[1].hreflang, "en");
    }

    // -----------------------------------------------------------------------
    // JSON-LD 结构化数据
    // -----------------------------------------------------------------------

    #[test]
    fn 提取json_ld() {
        let html = parse_html(r#"
            <html><head>
                <script type="application/ld+json">
                    {"@type": "WebPage", "name": "Test Page", "description": "A test"}
                </script>
            </head></html>
        "#);
        let data = extract_json_ld(&html);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].schema_type, Some("WebPage".to_string()));
        assert!(data[0].raw_json.is_some());
    }

    // -----------------------------------------------------------------------
    // 综合入口
    // -----------------------------------------------------------------------

    #[test]
    fn extract_metadata_完整提取() {
        let html = parse_html(r#"
            <html lang="zh-CN">
            <head>
                <meta charset="utf-8">
                <title>完整测试页面</title>
                <meta name="description" content="这是一个完整的测试页面">
                <meta name="keywords" content="测试, 元数据, 提取">
                <meta name="author" content="测试作者">
                <meta property="og:title" content="OG 标题">
                <meta property="og:type" content="website">
                <meta name="twitter:card" content="summary">
                <link rel="canonical" href="/canonical">
                <link rel="icon" href="/favicon.ico">
                <link rel="alternate" hreflang="en" href="/en/">
                <base href="https://example.com/blog/">
            </head>
            <body></body>
            </html>
        "#);
        let base = Url::parse("https://example.com").ok();
        let meta = extract_metadata(&html, base.as_ref()).unwrap();
        assert_eq!(meta.title, Some("OG 标题".to_string()));
        assert_eq!(meta.charset, Some("utf-8".to_string()));
        assert_eq!(meta.language, Some("zh-CN".to_string()));
        assert_eq!(meta.base_url, Some("https://example.com/blog/".to_string()));
        assert_eq!(meta.description, Some("这是一个完整的测试页面".to_string()));
        assert_eq!(meta.keywords, vec!["测试", "元数据", "提取"]);
        assert_eq!(meta.author, Some("测试作者".to_string()));
        assert_eq!(meta.canonical, Some("https://example.com/canonical".to_string()));
        assert!(meta.favicon.unwrap().contains("favicon.ico"));
        assert_eq!(meta.alternates.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 辅助函数
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_url_相对路径() {
        let base = Url::parse("https://example.com/path/").ok();
        assert_eq!(
            resolve_url(base.as_ref(), "/page.html"),
            Some("https://example.com/page.html".to_string())
        );
    }

    #[test]
    fn resolve_url_绝对路径() {
        let base = Url::parse("https://example.com/path/").ok();
        assert_eq!(
            resolve_url(base.as_ref(), "https://other.com/page"),
            Some("https://other.com/page".to_string())
        );
    }

    #[test]
    fn resolve_url_无效返回None() {
        let base = Url::parse("https://example.com").ok();
        assert_eq!(resolve_url(base.as_ref(), ""), None);
        assert_eq!(resolve_url(base.as_ref(), "data:image/png;base64,abc"), None);
        assert_eq!(resolve_url(base.as_ref(), "javascript:void(0)"), None);
    }

    #[test]
    fn find_largest_icon_选择最大() {
        let candidates = vec![
            ("/small.png".to_string(), "16x16".to_string()),
            ("/medium.png".to_string(), "32x32".to_string()),
            ("/large.png".to_string(), "64x64".to_string()),
        ];
        assert_eq!(find_largest_icon(&candidates), "/large.png");
    }

    #[test]
    fn find_largest_icon_无尺寸时选第一个() {
        let candidates = vec![
            ("/icon.png".to_string(), "".to_string()),
            ("/other.png".to_string(), "any".to_string()),
        ];
        assert_eq!(find_largest_icon(&candidates), "/icon.png");
    }

    #[test]
    fn parse_icon_size_标准格式() {
        assert_eq!(parse_icon_size("32x32"), Some(32));
        assert_eq!(parse_icon_size("64x32"), Some(64));
    }

    #[test]
    fn parse_icon_size_any返回None() {
        assert_eq!(parse_icon_size("any"), None);
        assert_eq!(parse_icon_size(""), None);
    }

    #[test]
    fn extract_meta_content_by_name() {
        let html = parse_html(r#"<html><head><meta name="generator" content="crawlkit"></head></html>"#);
        assert_eq!(extract_meta_content(&html, "generator"), Some("crawlkit".to_string()));
    }

    #[test]
    fn extract_meta_content_by_property() {
        let html = parse_html(r#"<html><head><meta property="article:section" content="tech"></head></html>"#);
        assert_eq!(extract_meta_content(&html, "article:section"), Some("tech".to_string()));
    }

    #[test]
    fn extract_meta_content_无匹配返回None() {
        let html = parse_html("<html><head></head></html>");
        assert_eq!(extract_meta_content(&html, "nonexistent"), None);
    }

    #[test]
    fn extract_custom_meta_过滤已知字段() {
        let html = parse_html(r#"
            <html><head>
                <meta name="description" content="skip me">
                <meta name="og:title" content="skip me too">
                <meta name="custom-field" content="keep me">
            </head></html>
        "#);
        let custom = extract_custom_meta(&html);
        assert_eq!(custom.len(), 1);
        assert_eq!(custom.get("custom-field"), Some(&"keep me".to_string()));
    }

    #[test]
    fn extract_microdata_基本提取() {
        let html = parse_html(r#"
            <html><body>
                <div itemscope itemtype="http://schema.org/Person">
                    <span itemprop="name">John Doe</span>
                    <span itemprop="email">john@example.com</span>
                </div>
            </body></html>
        "#);
        let data = extract_microdata(&html);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].schema_type, Some("Person".to_string()));
        assert_eq!(
            data[0].properties.get("name"),
            Some(&serde_json::Value::String("John Doe".to_string()))
        );
    }

    #[test]
    fn extract_og_property_存在() {
        let html = parse_html(r#"<html><head><meta property="og:image" content="https://example.com/img.jpg"></head></html>"#);
        assert_eq!(
            extract_og_property(&html, "og:image"),
            Some("https://example.com/img.jpg".to_string())
        );
    }

    #[test]
    fn extract_og_property_不存在返回None() {
        let html = parse_html("<html><head></head></html>");
        assert_eq!(extract_og_property(&html, "og:image"), None);
    }

    #[test]
    fn extract_twitter_property_存在() {
        let html = parse_html(r#"<html><head><meta name="twitter:image" content="https://example.com/img.jpg"></head></html>"#);
        assert_eq!(
            extract_twitter_property(&html, "twitter:image"),
            Some("https://example.com/img.jpg".to_string())
        );
    }

    #[test]
    fn 提取robots_高级指令() {
        let html = parse_html(r#"<html><head><meta name="robots" content="noindex, max-image-preview:standard, max-video-preview:30"></head></html>"#);
        let robots = extract_robots(&html);
        assert!(!robots.index);
        assert_eq!(robots.max_image_preview, Some("standard".to_string()));
        assert_eq!(robots.max_video_preview, 30);
    }

    #[test]
    fn extract_metadata_无base_url() {
        let html = parse_html(r#"
            <html><head>
                <title>No Base</title>
                <link rel="canonical" href="/page">
            </head></html>
        "#);
        let meta = extract_metadata(&html, None).unwrap();
        assert_eq!(meta.title, Some("No Base".to_string()));
    }

    #[test]
    fn extract_metadata_错误处理_无效选择器不崩溃() {
        let html = parse_html("<html><head><title>Safe</title></head></html>");
        let meta = extract_metadata(&html, None).unwrap();
        assert_eq!(meta.title, Some("Safe".to_string()));
    }
}
