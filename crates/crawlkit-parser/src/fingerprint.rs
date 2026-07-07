//! 内容指纹与 AMP 检测模块
//!
//! 提供内容指纹生成、变化检测、AMP 页面识别和缓存提示提取功能。
//! 改编自 halldyll-parser 的指纹与 AMP 检测实现。

use scraper::{Html, ElementRef, Node, Selector};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::selector::SELECTORS;

// ============================================================================
// 内容指纹
// ============================================================================

/// 内容指纹，用于检测 HTML 内容的变化
///
/// 存储文本哈希、结构哈希和元素统计信息，通过比较两个指纹来判断内容变化程度。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFingerprint {
    /// 全文文本哈希
    pub text_hash: u64,
    /// DOM 结构哈希
    pub structure_hash: u64,
    /// 主要正文文本哈希
    pub main_content_hash: u64,
    /// 元素总数
    pub element_count: usize,
    /// 文本节点数
    pub text_node_count: usize,
    /// 文本总长度（字符）
    pub text_length: usize,
    /// 主要正文文本长度
    pub main_content_length: usize,
}

impl ContentFingerprint {
    /// 判断与另一个指纹相比内容是否发生了变化
    ///
    /// 只要任一哈希不同，即认为内容已变化。
    pub fn has_changed(&self, other: &ContentFingerprint) -> bool {
        self.text_hash != other.text_hash || self.structure_hash != other.structure_hash
    }

    /// 判断是否仅发生了微小变化（文本改变但结构未变）
    pub fn has_minor_changes(&self, other: &ContentFingerprint) -> bool {
        self.text_hash != other.text_hash && self.structure_hash == other.structure_hash
    }

    /// 判断是否发生了结构性变化（DOM 树结构改变）
    pub fn has_structural_changes(&self, other: &ContentFingerprint) -> bool {
        self.structure_hash != other.structure_hash
    }

    /// 计算与另一个指纹的相似度（0.0 ~ 1.0）
    ///
    /// 综合考虑文本哈希、结构哈希和元素数量的差异。
    pub fn similarity(&self, other: &ContentFingerprint) -> f64 {
        let mut matching = 0.0;
        let mut total = 0.0;

        // 文本哈希相似度贡献 40%
        if self.text_hash == other.text_hash {
            matching += 40.0;
        } else {
            // 根据文本长度的接近程度给部分分数
            let min_len = self.text_length.min(other.text_length) as f64;
            let max_len = self.text_length.max(other.text_length) as f64;
            if max_len > 0.0 {
                matching += 40.0 * (min_len / max_len);
            }
        }
        total += 40.0;

        // 结构哈希相似度贡献 40%
        if self.structure_hash == other.structure_hash {
            matching += 40.0;
        } else if self.element_count > 0 || other.element_count > 0 {
            // 根据元素数量的接近程度给部分分数
            let min_el = self.element_count.min(other.element_count) as f64;
            let max_el = self.element_count.max(other.element_count) as f64;
            if max_el > 0.0 {
                matching += 40.0 * (min_el / max_el);
            }
        }
        total += 40.0;

        // 正文哈希相似度贡献 20%
        if self.main_content_hash == other.main_content_hash {
            matching += 20.0;
        } else {
            let min_mc = self.main_content_length.min(other.main_content_length) as f64;
            let max_mc = self.main_content_length.max(other.main_content_length) as f64;
            if max_mc > 0.0 {
                matching += 20.0 * (min_mc / max_mc);
            }
        }
        total += 20.0;

        if total == 0.0 {
            return 1.0;
        }
        matching / total
    }
}

// ============================================================================
// AMP 信息
// ============================================================================

/// AMP 页面信息
///
/// 存储从 HTML 文档中提取的 AMP 相关元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmpInfo {
    /// 当前页面是否为 AMP 页面
    pub is_amp: bool,
    /// AMP 版本的 URL（如果当前页面不是 AMP，这里有指向 AMP 版本的链接）
    pub amp_url: Option<String>,
    /// 标准版（canonical）URL
    pub canonical_url: Option<String>,
    /// 是否加载了 AMP 运行时
    pub has_amp_runtime: bool,
    /// 使用中的 AMP 扩展组件列表
    pub amp_components: Vec<String>,
    /// AMP 版本号
    pub amp_version: Option<String>,
}

impl AmpInfo {
    /// 是否包含 AMP 版本号信息
    pub fn has_amp_version(&self) -> bool {
        self.amp_version.is_some()
    }
}

// ============================================================================
// 缓存提示
// ============================================================================

/// 缓存提示信息
///
/// 从 HTTP 响应头或 HTML 中提取的缓存相关元数据。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheHints {
    /// 建议的缓存时间（秒）
    pub max_age: Option<u64>,
    /// 缓存过期后允许使用过期内容的宽限时间（秒）
    pub stale_while_revalidate: Option<u64>,
    /// ETag 标识
    pub etag: Option<String>,
    /// 最后修改时间
    pub last_modified: Option<String>,
    /// 是否应缓存此内容
    pub should_cache: bool,
    /// 缓存键建议
    pub cache_key: Option<String>,
}

// ============================================================================
// 指纹生成
// ============================================================================

/// 为 HTML 文档生成完整的内容指纹
pub fn generate_fingerprint(document: &Html) -> ContentFingerprint {
    let text = extract_text_only(document);
    let structure_hash = extract_structure(document);
    let main_content = extract_main_content(document);
    let text_hash = hash_string(&text);
    let main_content_hash = hash_string(&main_content);
    let element_count = count_elements(document);
    let text_node_count = count_text_nodes(document);

    ContentFingerprint {
        text_hash,
        structure_hash,
        main_content_hash,
        element_count,
        text_node_count,
        text_length: text.len(),
        main_content_length: main_content.len(),
    }
}

/// 为 HTML 文档生成指纹
///
/// `generate_fingerprint` 的别名，提供更直观的命名。
pub fn fingerprint_document(document: &Html) -> ContentFingerprint {
    generate_fingerprint(document)
}

/// 计算字符串的哈希值
pub fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// 从文档中提取主要正文内容
///
/// 先尝试使用 `article`、`main`、`[role=main]` 等选择器定位正文区域，
/// 如果都不匹配则回退到 `<body>`。
pub fn extract_main_content(document: &Html) -> String {
    let content_selectors = ["article", "main", "[role=main]", ".content", ".post-content"];

    for sel_str in &content_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = document.select(&sel).next() {
                let text = collect_text_recursive(el);
                if text.len() > 50 {
                    return text;
                }
            }
        }
    }

    // 回退到 body
    if let Ok(body_sel) = Selector::parse("body") {
        if let Some(body) = document.select(&body_sel).next() {
            return collect_text_recursive(body);
        }
    }

    // 最后回退到根元素
    collect_text_recursive(document.root_element())
}

/// 从文档中提取所有文本（不含 HTML 标签）
pub fn extract_text_only(document: &Html) -> String {
    collect_text_recursive(document.root_element())
}

/// 提取文档的结构指纹（基于 DOM 标签层次结构）
pub fn extract_structure(document: &Html) -> u64 {
    let mut structure = String::new();
    let root = document.root_element();
    extract_structure_recursive(&root, &mut structure, 0);
    hash_string(&structure)
}

/// 递归提取元素的 DOM 结构
fn extract_structure_recursive(element: &ElementRef, output: &mut String, depth: usize) {
    let tag_name = element.value().name.local.as_ref();
    output.push_str(&format!("<{}", tag_name));

    // 只保留 id 和少量关键属性以保持结构泛化能力
    if let Some(id) = element.value().attr("id") {
        output.push_str(&format!("#{}", id));
    }

    let tag = tag_name.to_lowercase();
    if tag == "a" {
        if let Some(href) = element.value().attr("href") {
            if href.starts_with("http") {
                output.push_str("[ext]");
            } else {
                output.push_str("[rel]");
            }
        } else {
            output.push_str("[no]");
        }
    } else if tag == "img" {
        output.push_str("[img]");
    }

    output.push_str(&format!(":{}", depth));

    for child in element.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            extract_structure_recursive(&child_el, output, depth + 1);
        }
    }
}

/// 统计文档中的 HTML 元素总数
pub fn count_elements(document: &Html) -> usize {
    count_elements_recursive(&document.root_element())
}

fn count_elements_recursive(element: &ElementRef) -> usize {
    let mut count = 1;
    for child in element.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            count += count_elements_recursive(&child_el);
        }
    }
    count
}

/// 统计文档中的文本节点数量
pub fn count_text_nodes(document: &Html) -> usize {
    count_text_nodes_recursive(&document.root_element())
}

fn count_text_nodes_recursive(element: &ElementRef) -> usize {
    let mut count = 0;
    for child in element.children() {
        match child.value() {
            Node::Text(t) => {
                let trimmed = t.text.trim();
                if !trimmed.is_empty() {
                    count += 1;
                }
            }
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    count += count_text_nodes_recursive(&child_el);
                }
            }
            _ => {}
        }
    }
    count
}

// ============================================================================
// AMP 检测
// ============================================================================

/// 从 HTML 文档中提取 AMP 信息
pub fn extract_amp_info(document: &Html) -> AmpInfo {
    let is_amp = detect_is_amp_page(document);
    let amp_url = extract_amp_link(document);
    let canonical_url = extract_canonical_link(document);
    let has_amp_runtime = detect_amp_runtime(document);
    let amp_components = extract_amp_components(document);
    let amp_version = detect_amp_version(document);

    AmpInfo {
        is_amp,
        amp_url,
        canonical_url,
        has_amp_runtime,
        amp_components,
        amp_version,
    }
}

/// 检测当前页面是否为 AMP 页面
///
/// 通过检查 `<html>` 标签是否包含 `amp` 或 `⚡` 属性来判断。
pub fn detect_is_amp_page(document: &Html) -> bool {
    if let Ok(html_sel) = Selector::parse("html") {
        if let Some(html_el) = document.select(&html_sel).next() {
            let el = html_el.value();
            if el.attr("amp").is_some() || el.has_class("amp", scraper::CaseSensitivity::CaseSensitive)
            {
                return true;
            }
            // 检查 ⚡ 属性
            if el.attr("\u{26A1}").is_some() {
                return true;
            }
        }
    }
    false
}

/// 提取 AMP 版本的链接（`<link rel="amphtml">`）
pub fn extract_amp_link(document: &Html) -> Option<String> {
    let sel_str = "link[rel=amphtml]";
    if let Ok(sel) = Selector::parse(sel_str) {
        if let Some(el) = document.select(&sel).next() {
            return el.value().attr("href").map(|s| s.to_string());
        }
    }
    None
}

/// 提取 canonical 链接（`<link rel="canonical">`）
pub fn extract_canonical_link(document: &Html) -> Option<String> {
    if let Some(el) = document.select(&SELECTORS.link).find(|e| {
        e.value().attr("rel").map_or(false, |r| {
            r.to_lowercase() == "canonical"
        })
    }) {
        return el.value().attr("href").map(|s| s.to_string());
    }
    None
}

/// 检测页面是否加载了 AMP 运行时
pub fn detect_amp_runtime(document: &Html) -> bool {
    let sel_str = "script[src*=ampproject]";
    if let Ok(sel) = Selector::parse(sel_str) {
        if document.select(&sel).any(|el| {
            el.value().attr("src").map_or(false, |src| {
                src.contains("cdn.ampproject.org")
            })
        }) {
            return true;
        }
    }
    false
}

/// 提取页面使用的 AMP 扩展组件列表
pub fn extract_amp_components(document: &Html) -> Vec<String> {
    let mut components = Vec::new();

    // 查找 script[custom-element] 和 script[custom-template]
    if let Ok(sel) = Selector::parse("script[custom-element], script[custom-template]") {
        for el in document.select(&sel) {
            let name = el.value().attr("custom-element")
                .or_else(|| el.value().attr("custom-template"))
                .map(|s| s.to_string());
            if let Some(n) = name {
                if !components.contains(&n) {
                    components.push(n);
                }
            }
        }
    }

    components
}

/// 检测 AMP 版本号
///
/// 从 AMP 运行时脚本的 `src` 属性中提取版本号。
pub fn detect_amp_version(document: &Html) -> Option<String> {
    let sel_str = "script[src*=cdn.ampproject]";
    if let Ok(sel) = Selector::parse(sel_str) {
        for el in document.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                // 版本号通常以 /v0.js 或 /v0/ 形式出现
                if let Some(ver_start) = src.rfind('/') {
                    let candidate = &src[ver_start + 1..];
                    if candidate == "v0.js" || candidate.starts_with("v0/") {
                        // 从 URL 中提取完整版本号
                        let parts: Vec<&str> = src.split('/').collect();
                        for part in &parts {
                            if part.starts_with("v0") && part.len() > 2 {
                                let ver = part[2..].trim_start_matches('-');
                                if !ver.is_empty() && ver != ".js" {
                                    return Some(ver.to_string());
                                }
                            }
                        }
                        return None;
                    }
                }
            }
        }
    }
    None
}

/// 解析相对 URL 为绝对 URL
///
/// 使用 `base_url` 作为基础，将 `relative` 解析为完整 URL。
pub fn resolve_url(base_url: &str, relative: &str) -> Option<String> {
    let base = url::Url::parse(base_url).ok()?;
    base.join(relative).ok().map(|u| u.to_string())
}

// ============================================================================
// 缓存提示提取
// ============================================================================

/// 从 HTML 文档中提取缓存提示信息
///
/// 检查 `<meta http-equiv>` 和缓存的启发式规则。
pub fn extract_cache_hints(document: &Html) -> CacheHints {
    let mut hints = CacheHints::default();
    let mut no_cache_set = false;

    // 检查 meta[http-equiv] 标签
    if let Ok(sel) = Selector::parse("meta[http-equiv]") {
        for el in document.select(&sel) {
            let equiv = el.value().attr("http-equiv")
                .map(|s| s.to_lowercase());
            let content = el.value().attr("content");

            match (equiv.as_deref(), content) {
                (Some("cache-control"), Some(val)) => {
                    if is_no_cache_value(val) {
                        hints.should_cache = false;
                        no_cache_set = true;
                    } else {
                        parse_cache_control_with_default(val, &mut hints);
                    }
                }
                (Some("expires"), Some(val)) => {
                    if hints.max_age.is_none() {
                        // 尝试解析过期时间
                        if let Some(expires) = parse_http_date(val) {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            if expires > now {
                                hints.max_age = Some(expires - now);
                            }
                        }
                    }
                }
                (Some("pragma"), Some(val)) => {
                    if val.to_lowercase().contains("no-cache") {
                        hints.should_cache = false;
                        no_cache_set = true;
                    }
                }
                (Some("last-modified"), Some(val)) => {
                    hints.last_modified = Some(val.to_string());
                }
                (Some("etag"), Some(val)) => {
                    hints.etag = Some(val.to_string());
                }
                _ => {}
            }
        }
    }

    // 默认情况下，如果没设置禁止缓存，就认为是可缓存的
    if !no_cache_set {
        hints.should_cache = true;
        if hints.max_age.is_none() {
            hints.max_age = Some(300);
        }
    }

    // 生成缓存键：使用页面标题和正文长度的组合
    if let Some(title_el) = document.select(&SELECTORS.title).next() {
        let title_text = title_el.text().collect::<String>();
        let clean_title = title_text.trim();
        if !clean_title.is_empty() {
            hints.cache_key = Some(format!("page-{}", hash_string(clean_title)));
        }
    }

    hints
}

fn is_no_cache_value(val: &str) -> bool {
    val.split(',')
        .map(|d| d.trim().to_lowercase())
        .any(|d| d == "no-cache" || d == "no-store" || d == "private")
}

fn parse_cache_control_with_default(val: &str, hints: &mut CacheHints) {
    for directive in val.split(',') {
        let d = directive.trim().to_lowercase();
        if let Some(max_age) = d.strip_prefix("max-age=") {
            hints.max_age = max_age.trim().parse().ok();
        } else if let Some(stale) = d.strip_prefix("stale-while-revalidate=") {
            hints.stale_while_revalidate = stale.trim().parse().ok();
        }
    }
}

fn parse_http_date(date_str: &str) -> Option<u64> {
    // 简单尝试解析 HTTP 日期格式（RFC 2822/1123）
    // 此处不做完整实现，返回 None 表示无法解析
    // 完整解析可依赖 httpdate 或 chrono crate
    let _ = date_str;
    None
}

// ============================================================================
// 便利函数
// ============================================================================

/// 检查两个 HTML 文档的内容是否发生了变化
///
/// 返回 `true` 表示内容已变化（需要重新处理）。
pub fn has_content_changed(old_doc: &Html, new_doc: &Html) -> bool {
    let old_fp = generate_fingerprint(old_doc);
    let new_fp = generate_fingerprint(new_doc);
    old_fp.has_changed(&new_fp)
}

/// 计算两个 HTML 文档的内容相似度
///
/// 返回 0.0 ~ 1.0 之间的相似度分数。
pub fn content_similarity(doc_a: &Html, doc_b: &Html) -> f64 {
    let fp_a = generate_fingerprint(doc_a);
    let fp_b = generate_fingerprint(doc_b);
    fp_a.similarity(&fp_b)
}

/// 检测 HTML 文档是否为 AMP 页面
pub fn is_amp_page(html_content: &str) -> bool {
    let doc = Html::parse_document(html_content);
    let info = extract_amp_info(&doc);
    info.is_amp
}

/// 获取 AMP 版本的 URL（如果存在）
///
/// 对于非 AMP 页面，返回指向其 AMP 版本的链接；
/// 对于 AMP 页面，返回其 canonical 链接。
pub fn get_amp_url(html_content: &str) -> Option<String> {
    let doc = Html::parse_document(html_content);
    let info = extract_amp_info(&doc);
    if info.is_amp {
        info.canonical_url
    } else {
        info.amp_url
    }
}

/// 快速计算 HTML 文档的文本哈希值
///
/// 比 `generate_fingerprint` 轻量，仅对全文文本进行哈希。
pub fn quick_html_hash(html_content: &str) -> u64 {
    let doc = Html::parse_document(html_content);
    let text = extract_text_only(&doc);
    hash_string(&text)
}

/// 快速计算文本内容的哈希值
///
/// 直接对纯文本进行哈希，不解析 HTML。
pub fn quick_hash(text: &str) -> u64 {
    hash_string(text)
}

// ============================================================================
// 辅助函数
// ============================================================================

fn collect_text_recursive(element: ElementRef) -> String {
    let mut result = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                let trimmed = text.text.trim();
                if !trimmed.is_empty() {
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.push_str(trimmed);
                }
            }
            Node::Element(el) => {
                let tag = el.name.local.as_ref();
                // 跳过 script、style、noscript 内容
                if matches!(tag, "script" | "style" | "noscript") {
                    continue;
                }
                if let Some(child_el) = ElementRef::wrap(child) {
                    let child_text = collect_text_recursive(child_el);
                    if !child_text.is_empty() {
                        if is_block_element(tag) && !result.is_empty() && !result.ends_with('\n') {
                            result.push('\n');
                        }
                        result.push_str(&child_text);
                        if is_block_element(tag) {
                            result.push('\n');
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result
}

fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "ul" | "ol" | "li" | "blockquote" | "pre" | "table"
            | "section" | "article" | "header" | "footer" | "nav"
            | "aside" | "br" | "hr" | "figure" | "figcaption"
            | "dl" | "dt" | "dd" | "tr" | "form"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 辅助函数 ─────────────────────────────────────────────

    fn create_doc(html: &str) -> Html {
        Html::parse_document(html)
    }

    // ─── 内容指纹测试 ──────────────────────────────────────────

    #[test]
    fn test_fingerprint_identical_html() {
        let html = "<html><body><p>Hello World</p></body></html>";
        let doc1 = create_doc(html);
        let doc2 = create_doc(html);
        let fp1 = generate_fingerprint(&doc1);
        let fp2 = generate_fingerprint(&doc2);
        assert_eq!(fp1.text_hash, fp2.text_hash);
        assert_eq!(fp1.structure_hash, fp2.structure_hash);
        assert!(!fp1.has_changed(&fp2));
        assert!((fp1.similarity(&fp2) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_fingerprint_different_content() {
        let doc1 = create_doc("<html><body><p>Original content</p></body></html>");
        let doc2 = create_doc("<html><body><p>Modified content here</p></body></html>");
        let fp1 = generate_fingerprint(&doc1);
        let fp2 = generate_fingerprint(&doc2);
        assert!(fp1.has_changed(&fp2));
        assert!(fp1.similarity(&fp2) < 1.0);
    }

    #[test]
    fn test_structural_change_detection() {
        let doc1 = create_doc("<html><body><p>Text</p></body></html>");
        let doc2 = create_doc("<html><body><div><p>Text</p></div></body></html>");
        let fp1 = generate_fingerprint(&doc1);
        let fp2 = generate_fingerprint(&doc2);
        assert!(fp1.has_structural_changes(&fp2));
        assert!(!fp1.has_minor_changes(&fp2));
    }

    #[test]
    fn test_minor_change_detection() {
        let doc1 = create_doc("<html><body><p>Hello World</p></body></html>");
        let doc2 = create_doc("<html><body><p>Hello World!</p></body></html>");
        let fp1 = generate_fingerprint(&doc1);
        let fp2 = generate_fingerprint(&doc2);
        assert!(fp1.has_minor_changes(&fp2));
        assert!(!fp1.has_structural_changes(&fp2));
    }

    #[test]
    fn test_fingerprint_empty_document() {
        let doc = create_doc("");
        let fp = generate_fingerprint(&doc);
        assert_eq!(fp.text_length, 0);
        assert!(fp.element_count > 0);
        assert_eq!(fp.text_node_count, 0);
    }

    #[test]
    fn test_hash_string_consistency() {
        let h1 = hash_string("hello");
        let h2 = hash_string("hello");
        let h3 = hash_string("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_fingerprint_document_alias() {
        let doc = create_doc("<html><body><p>Test</p></body></html>");
        let fp1 = fingerprint_document(&doc);
        let fp2 = generate_fingerprint(&doc);
        assert_eq!(fp1.text_hash, fp2.text_hash);
        assert_eq!(fp1.structure_hash, fp2.structure_hash);
    }

    // ─── 内容提取测试 ──────────────────────────────────────────

    #[test]
    fn test_extract_main_content_finds_article() {
        let html = r#"
            <html><body>
                <nav>Nav content</nav>
                <article><p>This is the main article content with enough text to exceed the minimum threshold of fifty characters so that it gets selected</p></article>
                <footer>Footer</footer>
            </body></html>
        "#;
        let doc = create_doc(html);
        let content = extract_main_content(&doc);
        assert!(content.contains("main article content"));
        assert!(!content.contains("Nav"));
    }

    #[test]
    fn test_extract_text_only_returns_all_text() {
        let html = "<html><body><h1>Title</h1><p>Paragraph</p></body></html>";
        let doc = create_doc(html);
        let text = extract_text_only(&doc);
        assert!(text.contains("Title"));
        assert!(text.contains("Paragraph"));
    }

    #[test]
    fn test_count_elements_basic() {
        let html = "<html><body><div><p>1</p><p>2</p></div></body></html>";
        let doc = create_doc(html);
        let count = count_elements(&doc);
        assert_eq!(count, 6);
    }

    #[test]
    fn test_count_text_nodes_basic() {
        let html = "<html><body><p>Hello</p><p>World</p></body></html>";
        let doc = create_doc(html);
        let count = count_text_nodes(&doc);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_extract_main_content_fallback_to_body() {
        let html = "<html><body><p>Just a simple body text</p></body></html>";
        let doc = create_doc(html);
        let content = extract_main_content(&doc);
        assert!(content.contains("simple body text"));
    }

    // ─── AMP 检测测试 ──────────────────────────────────────────

    #[test]
    fn test_detect_amp_page_with_amp_attribute() {
        let html = r#"<html amp><head><title>AMP Page</title></head><body>Hello</body></html>"#;
        let doc = create_doc(html);
        assert!(detect_is_amp_page(&doc));
    }

    #[test]
    fn test_detect_amp_page_with_emoji_attribute() {
        let html = "<html \u{26A1}><head><title>AMP</title></head><body>Hi</body></html>";
        let doc = create_doc(html);
        assert!(detect_is_amp_page(&doc));
    }

    #[test]
    fn test_detect_non_amp_page() {
        let html = "<html><head><title>Normal</title></head><body>Hi</body></html>";
        let doc = create_doc(html);
        assert!(!detect_is_amp_page(&doc));
    }

    #[test]
    fn test_extract_amp_link() {
        let html = r#"
            <head>
                <link rel="amphtml" href="https://example.com/amp/page">
            </head>
        "#;
        let doc = create_doc(html);
        assert_eq!(
            extract_amp_link(&doc),
            Some("https://example.com/amp/page".to_string())
        );
    }

    #[test]
    fn test_extract_canonical_link() {
        let html = r#"
            <head>
                <link rel="canonical" href="https://example.com/page">
            </head>
        "#;
        let doc = create_doc(html);
        assert_eq!(
            extract_canonical_link(&doc),
            Some("https://example.com/page".to_string())
        );
    }

    #[test]
    fn test_detect_amp_runtime_present() {
        let html = r#"
            <head>
                <script async src="https://cdn.ampproject.org/v0.js"></script>
            </head>
        "#;
        let doc = create_doc(html);
        assert!(detect_amp_runtime(&doc));
    }

    #[test]
    fn test_detect_amp_runtime_absent() {
        let html = "<html><head></head><body>No AMP</body></html>";
        let doc = create_doc(html);
        assert!(!detect_amp_runtime(&doc));
    }

    #[test]
    fn test_extract_amp_components() {
        let html = r#"
            <head>
                <script custom-element="amp-carousel" src="https://cdn.ampproject.org/v0/amp-carousel-0.1.js"></script>
                <script custom-element="amp-lightbox" src="https://cdn.ampproject.org/v0/amp-lightbox-0.1.js"></script>
                <script custom-template="amp-mustache" src="https://cdn.ampproject.org/v0/amp-mustache-0.2.js"></script>
            </head>
        "#;
        let doc = create_doc(html);
        let components = extract_amp_components(&doc);
        assert!(components.contains(&"amp-carousel".to_string()));
        assert!(components.contains(&"amp-lightbox".to_string()));
        assert!(components.contains(&"amp-mustache".to_string()));
        assert_eq!(components.len(), 3);
    }

    #[test]
    fn test_extract_amp_info_complete() {
        let html = r#"
            <html amp>
            <head>
                <title>AMP Test</title>
                <link rel="canonical" href="https://example.com/page">
                <script async src="https://cdn.ampproject.org/v0.js"></script>
                <script custom-element="amp-carousel" src="https://cdn.ampproject.org/v0/amp-carousel-0.1.js"></script>
            </head>
            <body><p>AMP content</p></body>
            </html>
        "#;
        let doc = create_doc(html);
        let info = extract_amp_info(&doc);
        assert!(info.is_amp);
        assert!(info.has_amp_runtime);
        assert_eq!(info.canonical_url, Some("https://example.com/page".to_string()));
        // v0.js 中无显式版本号，has_amp_version 可能为 false
    }

    #[test]
    fn test_amp_info_no_amp_version() {
        let html = r#"<html><head><title>Normal</title></head><body>No AMP</body></html>"#;
        let doc = create_doc(html);
        let info = extract_amp_info(&doc);
        assert!(!info.is_amp);
        assert!(!info.has_amp_version());
    }

    #[test]
    fn test_is_amp_page_convenience() {
        let amp_html = r#"<html amp><head><title>A</title></head><body>B</body></html>"#;
        assert!(is_amp_page(amp_html));
        let normal_html = "<html><head><title>A</title></head><body>B</body></html>";
        assert!(!is_amp_page(normal_html));
    }

    #[test]
    fn test_get_amp_url_from_non_amp() {
        let html = r#"
            <head>
                <link rel="amphtml" href="https://example.com/amp">
                <link rel="canonical" href="https://example.com/page">
            </head>
        "#;
        assert_eq!(get_amp_url(html), Some("https://example.com/amp".to_string()));
    }

    #[test]
    fn test_get_amp_url_from_amp() {
        let html = r#"
            <html amp>
            <head>
                <link rel="canonical" href="https://example.com/page">
            </head>
            <body>AMP</body>
            </html>
        "#;
        assert_eq!(get_amp_url(html), Some("https://example.com/page".to_string()));
    }

    // ─── URL 解析测试 ──────────────────────────────────────────

    #[test]
    fn test_resolve_absolute_url() {
        assert_eq!(
            resolve_url("https://example.com", "/path/to/page"),
            Some("https://example.com/path/to/page".to_string())
        );
    }

    #[test]
    fn test_resolve_relative_url() {
        assert_eq!(
            resolve_url("https://example.com/base/", "relative"),
            Some("https://example.com/base/relative".to_string())
        );
    }

    #[test]
    fn test_resolve_invalid_base_url() {
        assert_eq!(resolve_url("not-a-url", "/path"), None);
    }

    // ─── 缓存提示测试 ──────────────────────────────────────────

    #[test]
    fn test_cache_hints_from_meta_cache_control() {
        let html = r#"
            <head>
                <meta http-equiv="Cache-Control" content="max-age=3600">
            </head>
        "#;
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert!(hints.should_cache);
        assert_eq!(hints.max_age, Some(3600));
    }

    #[test]
    fn test_cache_hints_no_cache_directive() {
        let html = r#"
            <head>
                <meta http-equiv="Cache-Control" content="no-cache, no-store">
            </head>
        "#;
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert!(!hints.should_cache);
    }

    #[test]
    fn test_cache_hints_default_values() {
        let html = "<html><head><title>Test</title></head><body>Hello</body></html>";
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert!(hints.should_cache);
        assert_eq!(hints.max_age, Some(300));
    }

    #[test]
    fn test_cache_hints_with_etag() {
        let html = r#"
            <head>
                <meta http-equiv="ETag" content="abc123">
                <meta http-equiv="Cache-Control" content="max-age=7200">
            </head>
        "#;
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert_eq!(hints.etag, Some("abc123".to_string()));
        assert_eq!(hints.max_age, Some(7200));
    }

    #[test]
    fn test_cache_hints_cache_key_generated() {
        let html = "<html><head><title>My Article Title</title></head><body>Content</body></html>";
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert!(hints.cache_key.is_some());
        let key = hints.cache_key.unwrap();
        assert!(key.starts_with("page-"));
    }

    #[test]
    fn test_cache_hints_with_stale_revalidate() {
        let html = r#"
            <head>
                <meta http-equiv="Cache-Control" content="max-age=3600, stale-while-revalidate=86400">
            </head>
        "#;
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert_eq!(hints.stale_while_revalidate, Some(86400));
    }

    // ─── 便利函数测试 ──────────────────────────────────────────

    #[test]
    fn test_has_content_changed_true() {
        let old = create_doc("<html><body><p>Old content</p></body></html>");
        let new = create_doc("<html><body><p>New content</p></body></html>");
        assert!(has_content_changed(&old, &new));
    }

    #[test]
    fn test_has_content_changed_false() {
        let old = create_doc("<html><body><p>Same content</p></body></html>");
        let new = create_doc("<html><body><p>Same content</p></body></html>");
        assert!(!has_content_changed(&old, &new));
    }

    #[test]
    fn test_content_similarity_identical() {
        let doc = create_doc("<html><body><p>Hello World</p></body></html>");
        let sim = content_similarity(&doc, &doc);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_content_similarity_completely_different() {
        let doc1 = create_doc("<html><body><p>AAAA</p></body></html>");
        let doc2 = create_doc("<html><body><div><span>BBBB</span></div></body></html>");
        let sim = content_similarity(&doc1, &doc2);
        assert!(sim < 1.0);
        assert!(sim >= 0.0);
    }

    #[test]
    fn test_quick_hash_consistency() {
        let h1 = quick_hash("test data");
        let h2 = quick_hash("test data");
        let h3 = quick_hash("different data");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_quick_hash_empty() {
        let h = quick_hash("");
        assert_ne!(h, 0);
    }

    // ─── 边界情况测试 ──────────────────────────────────────────

    #[test]
    fn test_document_with_only_whitespace() {
        let doc = create_doc("<html><body>   \n   </body></html>");
        let fp = generate_fingerprint(&doc);
        assert_eq!(fp.text_length, 0);
        assert_eq!(fp.text_node_count, 0);
    }

    #[test]
    fn test_document_with_script_style_excluded() {
        let html = r#"
            <html><body>
                <p>Visible text</p>
                <script>var x = 1;</script>
                <style>.hidden {}</style>
            </body></html>
        "#;
        let doc = create_doc(html);
        let text = extract_text_only(&doc);
        assert!(text.contains("Visible text"));
        assert!(!text.contains("var x"));
        assert!(!text.contains(".hidden"));
    }

    #[test]
    fn test_fingerprint_similarity_same_structure_different_text() {
        let doc1 = create_doc("<html><body><p>Short</p></body></html>");
        let doc2 = create_doc("<html><body><p>Longer text here</p></body></html>");
        let fp1 = generate_fingerprint(&doc1);
        let fp2 = generate_fingerprint(&doc2);
        // 结构相同，文本不同，相似度应 > 0.4（结构分）
        assert!(fp1.similarity(&fp2) > 0.39);
    }

    #[test]
    fn test_nested_element_count() {
        let html = r#"
            <html><body>
                <div>
                    <ul>
                        <li>1</li>
                        <li>2</li>
                        <li>3</li>
                    </ul>
                </div>
            </body></html>
        "#;
        let doc = create_doc(html);
        let count = count_elements(&doc);
        assert_eq!(count, 8);
    }

    #[test]
    fn test_extract_amp_link_no_amp() {
        let html = "<html><head><title>Normal</title></head><body>Hi</body></html>";
        let doc = create_doc(html);
        assert_eq!(extract_amp_link(&doc), None);
    }

    #[test]
    fn test_extract_canonical_link_missing() {
        let html = "<html><head><title>No canonical</title></head><body>Hi</body></html>";
        let doc = create_doc(html);
        assert_eq!(extract_canonical_link(&doc), None);
    }

    #[test]
    fn test_amp_components_empty_when_no_amp() {
        let html = "<html><head><title>Normal</title></head><body>Hi</body></html>";
        let doc = create_doc(html);
        let components = extract_amp_components(&doc);
        assert!(components.is_empty());
    }

    #[test]
    fn test_cache_hints_last_modified() {
        let html = r#"
            <head>
                <meta http-equiv="Last-Modified" content="Mon, 01 Jan 2024 00:00:00 GMT">
            </head>
        "#;
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert!(hints.last_modified.is_some());
    }

    #[test]
    fn test_cache_hints_pragma_no_cache() {
        let html = r#"
            <head>
                <meta http-equiv="Pragma" content="no-cache">
            </head>
        "#;
        let doc = create_doc(html);
        let hints = extract_cache_hints(&doc);
        assert!(!hints.should_cache);
    }
}
