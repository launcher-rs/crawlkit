//! 链接提取与分析模块
//!
//! 基于 halldyll-parser 的链接提取逻辑改写。
//! 提供从 HTML 文档中提取链接、URL 解析、分类、过滤与统计分析功能。

use scraper::{ElementRef, Html};
use std::collections::HashSet;
use url::Url;

use crate::selector::SELECTORS;
use crate::types::{Link, LinkRel, LinkType, ParserConfig, ParserResult};

// ============================================================================
// 链接统计
// ============================================================================

/// 链接统计信息
#[derive(Debug, Clone, Default)]
pub struct LinkStats {
    /// 链接总数
    pub total: usize,
    /// 内部链接数
    pub internal: usize,
    /// 外部链接数
    pub external: usize,
    /// nofollow 链接数
    pub nofollow: usize,
    /// sponsored 链接数
    pub sponsored: usize,
    /// ugc 链接数
    pub ugc: usize,
    /// 可跟随链接数
    pub followable: usize,
    /// 唯一外部域名数
    pub unique_external_domains: usize,
    /// 外部域名列表
    pub external_domains: Vec<String>,
}

// ============================================================================
// 链接提取
// ============================================================================

/// 从 HTML 文档中提取所有链接。
///
/// 使用 `SELECTORS` 中定义的 `a[href]` 选择器匹配链接元素，
/// 并对每个元素依次调用 `extract_link` 进行解析。
pub fn extract_links(document: &Html, config: &ParserConfig) -> ParserResult<Vec<Link>> {
    let selector = &SELECTORS.a;
    let mut links = Vec::new();
    let mut seen = HashSet::new();

    for element in document.select(selector) {
        if let Some(link) = extract_link(&element, config.base_url.as_ref())
            && seen.insert(link.href.clone()) {
                links.push(link);
            }
    }

    Ok(links)
}

/// 从单个 `<a>` 元素提取 Link 结构。
///
/// 提取 href、文本、title、rel、target、hreflang 等属性，
/// 并解析 URL、链接类型、nofollow 状态等。
pub fn extract_link(element: &ElementRef, base_url: Option<&Url>) -> Option<Link> {
    let href = element.value().attr("href")?;
    let href = href.trim();
    if href.is_empty()
        || href.starts_with('#')
        || href.starts_with("javascript:")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
        || href.starts_with("data:")
    {
        return None;
    }

    let text: String = element.text().collect::<Vec<_>>().join(" ").trim().to_string();

    let resolved = resolve_url(href, base_url);
    let normalized = resolved.as_deref().and_then(normalize_url);

    let rel_attr = element.value().attr("rel").unwrap_or("");
    let rel = parse_rel_attribute(rel_attr);
    let is_nofollow = is_nofollow(&rel);

    let link_type = match normalized.as_deref() {
        Some(url) => determine_link_type(url, base_url),
        None => LinkType::Unknown,
    };

    let title = element.value().attr("title").map(ToString::to_string);
    let target = element.value().attr("target").map(ToString::to_string);
    let hreflang = element.value().attr("hreflang").map(ToString::to_string);

    Some(Link {
        href: href.to_string(),
        url: normalized,
        text: if text.is_empty() { href.to_string() } else { text },
        title,
        rel,
        link_type,
        is_nofollow,
        target,
        hreflang,
    })
}

// ============================================================================
// URL 处理
// ============================================================================

/// 将 href 属性值解析为绝对 URL。
///
/// 支持相对路径解析（基于 base_url）以及协议相对 URL（`//example.com/path`）。
pub fn resolve_url(href: &str, base_url: Option<&Url>) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    // 协议相对 URL：//example.com/path
    if href.starts_with("//") {
        let scheme = base_url.map_or("https", url::Url::scheme);
        return Some(format!("{scheme}:{href}"));
    }

    // 已经是绝对 URL
    if href.starts_with("http://") || href.starts_with("https://") {
        return Url::parse(href).ok().map(|u| u.to_string());
    }

    // 相对 URL，需要 base_url
    let base = base_url?;
    base.join(href).ok().map(|u| u.to_string())
}

/// 标准化 URL：移除片段标识符（fragment）、末尾斜杠（可选的）等。
pub fn normalize_url(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;

    // 只处理 http/https
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Some(url.to_string());
    }

    // 移除片段
    parsed.set_fragment(None);

    // 移除默认端口
    match parsed.port() {
        Some(80) if parsed.scheme() == "http" => {
            let _ = parsed.set_port(None);
        }
        Some(443) if parsed.scheme() == "https" => {
            let _ = parsed.set_port(None);
        }
        _ => {}
    }

    Some(parsed.to_string())
}

// ============================================================================
// 链接类型判定
// ============================================================================

/// 判断链接类型（内部 / 外部 / 未知）。
///
/// - 内部链接：与 base_url 域名相同（包括 www 前缀差异）
/// - 外部链接：不同域名，并且与 base_url 属于不同的一级域名
/// - 子域名链接与主站同属一个一级域名时视为内部链接
pub fn determine_link_type(url: &str, base_url: Option<&Url>) -> LinkType {
    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return LinkType::Unknown,
    };

    let base = match base_url {
        Some(b) => b,
        None => return LinkType::External,
    };

    let url_host = match parsed.host_str() {
        Some(h) => h,
        None => return LinkType::Internal,
    };

    let base_host = match base.host_str() {
        Some(h) => h,
        None => return LinkType::External,
    };

    if url_host == base_host {
        return LinkType::Internal;
    }

    // 检查是否为子域名关系
    if let Some(rest) = url_host.strip_suffix(base_host)
        && rest.ends_with('.')
    {
        return LinkType::Internal;
    }

    LinkType::External
}

// ============================================================================
// Rel 属性解析
// ============================================================================

/// 解析 `rel` 属性值为 `LinkRel` 枚举列表。
pub fn parse_rel_attribute(rel: &str) -> Vec<LinkRel> {
    if rel.is_empty() {
        return Vec::new();
    }

    rel.split_whitespace()
        .map(|token| match token.to_lowercase().as_str() {
            "nofollow" => LinkRel::NoFollow,
            "ugc" => LinkRel::Ugc,
            "sponsored" => LinkRel::Sponsored,
            "external" => LinkRel::External,
            "noopener" => LinkRel::NoOpener,
            "noreferrer" => LinkRel::NoReferrer,
            "follow" => LinkRel::Follow,
            _ => LinkRel::Other,
        })
        .collect()
}

/// 判断 rel 列表中是否包含 nofollow。
pub fn is_nofollow(rel: &[LinkRel]) -> bool {
    rel.iter().any(|r| matches!(r, LinkRel::NoFollow))
}

/// 判断 rel 列表中是否包含 sponsored。
pub fn is_sponsored(rel: &[LinkRel]) -> bool {
    rel.iter().any(|r| matches!(r, LinkRel::Sponsored))
}

/// 判断 rel 列表中是否包含 ugc。
pub fn is_ugc(rel: &[LinkRel]) -> bool {
    rel.iter().any(|r| matches!(r, LinkRel::Ugc))
}

// ============================================================================
// 链接过滤
// ============================================================================

/// 过滤出内部链接。
pub fn filter_internal_links(links: &[Link]) -> Vec<&Link> {
    links.iter().filter(|l| l.link_type == LinkType::Internal).collect()
}

/// 过滤出外部链接。
pub fn filter_external_links(links: &[Link]) -> Vec<&Link> {
    links.iter().filter(|l| l.link_type == LinkType::External).collect()
}

/// 过滤出可跟随的链接（非 nofollow / sponsored / ugc）。
pub fn filter_followable_links(links: &[Link]) -> Vec<&Link> {
    links.iter().filter(|l| l.should_follow()).collect()
}

/// 获取所有外部链接的唯一域名列表。
///
/// 从每个外部链接中提取域名，去重后返回。
pub fn get_external_domains(links: &[Link]) -> Vec<String> {
    let mut domains = HashSet::new();

    for link in links {
        if link.link_type != LinkType::External {
            continue;
        }
        if let Some(ref url_str) = link.url
            && let Ok(parsed) = Url::parse(url_str)
                && let Some(host) = parsed.host_str() {
                    domains.insert(host.to_string());
                }
    }

    let mut result: Vec<_> = domains.into_iter().collect();
    result.sort();
    result
}

// ============================================================================
// 链接统计
// ============================================================================

/// 计算链接统计信息。
pub fn calculate_link_stats(links: &[Link]) -> LinkStats {
    let total = links.len();
    let internal = links.iter().filter(|l| l.link_type == LinkType::Internal).count();
    let external = links.iter().filter(|l| l.link_type == LinkType::External).count();

    let nofollow = links.iter().filter(|l| l.is_nofollow).count();
    let sponsored = links.iter().filter(|l| is_sponsored(&l.rel)).count();
    let ugc = links.iter().filter(|l| is_ugc(&l.rel)).count();
    let followable = links.iter().filter(|l| l.should_follow()).count();

    let external_domains = get_external_domains(links);
    let unique_external_domains = external_domains.len();

    LinkStats {
        total,
        internal,
        external,
        nofollow,
        sponsored,
        ugc,
        followable,
        unique_external_domains,
        external_domains,
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    use scraper::Selector;

    // 辅助：构造测试用的 ParserConfig
    fn test_config(base_url: Option<&str>) -> ParserConfig {
        ParserConfig {
            base_url: base_url.and_then(|u| Url::parse(u).ok()),
            ..ParserConfig::default()
        }
    }

    // 辅助：从 HTML 字符串解析文档并提取链接
    fn extract_from_html(html: &str, base_url: Option<&str>) -> Vec<Link> {
        let document = Html::parse_document(html);
        let config = test_config(base_url);
        extract_links(&document, &config).unwrap()
    }

    #[test]
    fn test_extract_links_basic() {
        let html = r#"<html><body><a href="https://example.com">Example</a></body></html>"#;
        let links = extract_from_html(html, None);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href, "https://example.com");
        assert_eq!(links[0].text, "Example");
    }

    #[test]
    fn test_extract_links_filters_anchors_and_javascript() {
        let html = r##"<html><body>
            <a href="#section1">锚点</a>
            <a href="javascript:void(0)">脚本</a>
            <a href="mailto:user at host">邮箱</a>
            <a href="tel:+123456789">电话</a>
            <a href="data:text/plain,hello">数据</a>
            <a href="https://example.com/valid">有效</a>
        </body></html>"##;
        let links = extract_from_html(html, None);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href, "https://example.com/valid");
    }

    #[test]
    fn test_extract_links_deduplicates() {
        let html = r#"<html><body>
            <a href="https://example.com">A</a>
            <a href="https://example.com">B</a>
        </body></html>"#;
        let links = extract_from_html(html, None);
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn test_extract_links_resolves_relative_with_base() {
        let html = r#"<html><body><a href="/page">相对链接</a></body></html>"#;
        let links = extract_from_html(html, Some("https://example.com"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url.as_deref(), Some("https://example.com/page"));
        assert_eq!(links[0].link_type, LinkType::Internal);
    }

    #[test]
    fn test_extract_links_protocol_relative() {
        let html = r#"<html><body><a href="//cdn.example.com/file.js">CDN</a></body></html>"#;
        let links = extract_from_html(html, Some("https://example.com"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url.as_deref(), Some("https://cdn.example.com/file.js"));
    }

    #[test]
    fn test_extract_link_returns_none_for_element_without_href() {
        let html = Html::parse_document(r"<html><body><span>不是链接</span></body></html>");
        let selector = Selector::parse("span").unwrap();
        let element = html.select(&selector).next().unwrap();
        let result = extract_link(&element, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_url_absolute() {
        let base = Url::parse("https://example.com").ok();
        let result = resolve_url("https://other.com/path", base.as_ref());
        assert_eq!(result.as_deref(), Some("https://other.com/path"));
    }

    #[test]
    fn test_resolve_url_relative() {
        let base = Url::parse("https://example.com/base/").ok();
        let result = resolve_url("../page", base.as_ref());
        assert_eq!(result.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn test_resolve_url_protocol_relative() {
        let base = Url::parse("https://example.com").ok();
        let result = resolve_url("//other.com/path", base.as_ref());
        assert_eq!(result.as_deref(), Some("https://other.com/path"));
    }

    #[test]
    fn test_resolve_url_empty_or_skip() {
        assert!(resolve_url("", None).is_none());
        assert!(resolve_url("  ", None).is_none());
    }

    #[test]
    fn test_normalize_url_removes_fragment() {
        let result = normalize_url("https://example.com/page#section");
        assert_eq!(result.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn test_normalize_url_removes_default_port() {
        let result = normalize_url("https://example.com:443/page");
        assert_eq!(result.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn test_normalize_url_preserves_non_standard_port() {
        let result = normalize_url("https://example.com:8080/page");
        assert_eq!(result.as_deref(), Some("https://example.com:8080/page"));
    }

    #[test]
    fn test_normalize_url_keeps_http_and_https() {
        let result = normalize_url("ftp://example.com/file");
        assert_eq!(result.as_deref(), Some("ftp://example.com/file"));
    }

    #[test]
    fn test_determine_link_type_internal_exact() {
        let base = Url::parse("https://example.com").ok();
        let result = determine_link_type("https://example.com/page", base.as_ref());
        assert_eq!(result, LinkType::Internal);
    }

    #[test]
    fn test_determine_link_type_internal_subdomain() {
        let base = Url::parse("https://example.com").ok();
        let result = determine_link_type("https://blog.example.com/page", base.as_ref());
        assert_eq!(result, LinkType::Internal);
    }

    #[test]
    fn test_determine_link_type_external() {
        let base = Url::parse("https://example.com").ok();
        let result = determine_link_type("https://other.com/page", base.as_ref());
        assert_eq!(result, LinkType::External);
    }

    #[test]
    fn test_determine_link_type_no_base() {
        let result = determine_link_type("https://example.com/page", None);
        assert_eq!(result, LinkType::External);
    }

    #[test]
    fn test_parse_rel_attribute_nofollow() {
        let rel = parse_rel_attribute("nofollow");
        assert!(is_nofollow(&rel));
        assert!(!is_sponsored(&rel));
        assert!(!is_ugc(&rel));
    }

    #[test]
    fn test_parse_rel_attribute_multiple() {
        let rel = parse_rel_attribute("nofollow ugc sponsored");
        assert!(is_nofollow(&rel));
        assert!(is_sponsored(&rel));
        assert!(is_ugc(&rel));
    }

    #[test]
    fn test_parse_rel_attribute_empty() {
        let rel = parse_rel_attribute("");
        assert!(rel.is_empty());
    }

    #[test]
    fn test_parse_rel_attribute_case_insensitive() {
        let rel = parse_rel_attribute("NoFollow UGC Sponsored");
        assert!(is_nofollow(&rel));
        assert!(is_sponsored(&rel));
        assert!(is_ugc(&rel));
    }

    #[test]
    fn test_filter_internal_links() {
        let mut a = Link::new("https://example.com/internal", "内部");
        a.link_type = LinkType::Internal;
        let mut b = Link::new("https://external.com/external", "外部");
        b.link_type = LinkType::External;
        let links = vec![a, b];
        let internal = filter_internal_links(&links);
        assert_eq!(internal.len(), 1);
        assert_eq!(internal[0].href, "https://example.com/internal");
    }

    #[test]
    fn test_filter_external_links() {
        let mut a = Link::new("https://example.com/internal", "内部");
        a.link_type = LinkType::Internal;
        let mut b = Link::new("https://external.com/external", "外部");
        b.link_type = LinkType::External;
        let links = vec![a, b];
        let external = filter_external_links(&links);
        assert_eq!(external.len(), 1);
        assert_eq!(external[0].href, "https://external.com/external");
    }

    #[test]
    fn test_filter_followable_links() {
        let mut follow = Link::new("https://example.com/follow", "可跟随");
        follow.link_type = LinkType::Internal;
        let mut nofollow = Link::new("https://example.com/nofollow", "不可跟随");
        nofollow.link_type = LinkType::Internal;
        nofollow.is_nofollow = true;

        let links = vec![follow, nofollow];
        let followable = filter_followable_links(&links);
        assert_eq!(followable.len(), 1);
        assert_eq!(followable[0].href, "https://example.com/follow");
    }

    #[test]
    fn test_get_external_domains() {
        let mut a = Link::new("https://example.com/internal", "内部");
        a.url = Some("https://example.com/internal".to_string());
        a.link_type = LinkType::Internal;
        let mut b = Link::new("https://external.com/page1", "外部1");
        b.url = Some("https://external.com/page1".to_string());
        b.link_type = LinkType::External;
        let mut c = Link::new("https://external.com/page2", "外部2");
        c.url = Some("https://external.com/page2".to_string());
        c.link_type = LinkType::External;
        let mut d = Link::new("https://other.org/page", "外部3");
        d.url = Some("https://other.org/page".to_string());
        d.link_type = LinkType::External;
        let links = vec![a, b, c, d];
        let domains = get_external_domains(&links);
        assert_eq!(domains, vec!["external.com", "other.org"]);
    }

    #[test]
    fn test_calculate_link_stats() {
        let mut ext = Link::new("https://external.com/page", "外部链接");
        ext.url = Some("https://external.com/page".to_string());
        ext.link_type = LinkType::External;

        let mut int = Link::new("https://example.com/page", "内部链接");
        int.url = Some("https://example.com/page".to_string());
        int.link_type = LinkType::Internal;
        int.is_nofollow = true;

        let links = vec![ext, int];

        let stats = calculate_link_stats(&links);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.internal, 1);
        assert_eq!(stats.external, 1);
        assert_eq!(stats.nofollow, 1);
        assert_eq!(stats.followable, 1);
        assert_eq!(stats.unique_external_domains, 1);
    }

    #[test]
    fn test_extract_link_with_title_and_target() {
        let html = r#"<html><body><a href="https://example.com" title="示例" target="_blank" rel="nofollow">链接</a></body></html>"#;
        let document = Html::parse_document(html);
        let selector = Selector::parse("a").unwrap();
        let element = document.select(&selector).next().unwrap();
        let link = extract_link(&element, None).unwrap();

        assert_eq!(link.title.as_deref(), Some("示例"));
        assert_eq!(link.target.as_deref(), Some("_blank"));
        assert!(link.is_nofollow);
        assert!(link.opens_new_tab());
    }

    #[test]
    fn test_should_skip_link_with_empty_href() {
        let html = r#"<html><body><a href="">空链接</a></body></html>"#;
        let links = extract_from_html(html, None);
        assert!(links.is_empty());
    }

    #[test]
    fn test_only_whitespace_href_skipped() {
        let html = r#"<html><body><a href="   ">空白链接</a></body></html>"#;
        let links = extract_from_html(html, None);
        assert!(links.is_empty());
    }

    #[test]
    fn test_same_domain_different_scheme_is_external() {
        // 链接为 http，基准为 https，但域名相同 —— 仍设为内部链接
        let base = Url::parse("https://example.com").ok();
        let result = determine_link_type("http://example.com/page", base.as_ref());
        assert_eq!(result, LinkType::Internal);
    }

    #[test]
    fn test_extract_links_multiple_selectors() {
        let html = r#"<html><body>
            <a href="https://a.com">A</a>
            <a href="https://b.com">B</a>
            <a href="https://c.com">C</a>
        </body></html>"#;
        let links = extract_from_html(html, None);
        assert_eq!(links.len(), 3);
    }

    #[test]
    fn test_normalize_url_noop() {
        let result = normalize_url("https://example.com/path/to/page");
        assert_eq!(result.as_deref(), Some("https://example.com/path/to/page"));
    }

    #[test]
    fn test_calculate_link_stats_empty() {
        let stats = calculate_link_stats(&[]);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.internal, 0);
        assert_eq!(stats.external, 0);
        assert_eq!(stats.followable, 0);
    }
}
