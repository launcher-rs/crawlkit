//! 基于 DOM 结构聚类 + 多信号打分的文章链接提取器。
//!
//! 与纯正则方案（见 `rules` 模块）的区别：
//! - 正则只能判断"这个 URL 长得像不像文章"，遇到 aei.org / newamerica.org
//!   这种纯 slug 结构（`/program/area/slug`）就完全失效。
//! - 本模块从页面结构入手：真正的文章列表在 DOM 里通常是"同一父级路径下
//!   重复出现的相似节点"（比如同一个 `<ul class="news-list">` 下的十几个
//!   `<li><a>`）。把链接按其父级路径签名聚类，聚类越大，越可能是文章列表。
//! - 再结合锚文本长度、URL 路径深度、导航词过滤、以及复用 UrlRule 的正则
//!   规则，做加权打分，避免"一条正则命中/不命中"的非黑即白判定。
//!
//! # 示例
//!
//! ```rust
//! use crawlkit_parser::{LinkExtractor, ExtractorConfig};
//!
//! let html = r#"
//! <html><body>
//!     <div class="news-list">
//!         <a href="/2024/01/15/story-one">第一条新闻标题</a>
//!         <a href="/2024/01/16/story-two">第二条新闻标题</a>
//!         <a href="/2024/01/17/story-three">第三条新闻标题</a>
//!         <a href="/2024/01/18/story-four">第四条新闻标题</a>
//!     </div>
//! </body></html>
//! "#;
//!
//! let extractor = LinkExtractor::new(ExtractorConfig::default());
//! let results = extractor.extract(html, "https://example.com/");
//!
//! assert_eq!(results.len(), 4);
//! for link in &results {
//!     println!("{} (score: {})", link.url, link.score);
//! }
//! ```
//!

use std::collections::HashMap;

use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::rules::UrlRule;

/// 提取到的候选文章链接。
///
/// 包含 URL、锚文本、综合得分以及所在列表结构的聚类大小。
///
/// ```rust
/// use crawlkit_parser::{LinkExtractor, ExtractorConfig};
///
/// let html = r#"<a href="/2024/01/15/story">文章标题</a>"#;
/// let extractor = LinkExtractor::new(ExtractorConfig::default());
/// let results = extractor.extract(html, "https://example.com/");
/// for link in &results {
///     println!("{} | score={} | cluster={}", link.url, link.score, link.cluster_size);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ExtractedLink {
    /// 完整 URL（已解析为绝对地址）
    pub url: String,
    /// 锚文本（已清理多余空白）
    pub text: String,
    /// 综合评分（8 个信号的加权和，越高越可能是文章链接）
    pub score: f32,
    /// 该链接所在 DOM 路径下同类链接的数量（列表结构强度）
    pub cluster_size: usize,
}

/// 提取器配置，按需调整以适配不同站点。
///
/// # 默认值
///
/// | 字段 | 默认值 | 说明 |
/// |------|--------|------|
/// | `score_threshold` | `1.5` | 低于此分的链接会被过滤 |
/// | `cluster_depth` | `4` | DOM 路径签名向上遍历层数 |
/// | `cluster_min_size` | `4` | 聚类大小 >= 此值视为强信号 |
/// | `min_text_len` | `8` | 锚文本短于此值扣分 |
/// | `use_url_rule` | `true` | 是否开启 UrlRule 辅助信号 |
/// | `non_article_paths` | 24 条默认路径 | 含 `/people/`、`/about/` 等 |
///
/// ```rust
/// use crawlkit_parser::ExtractorConfig;
///
/// // 低阈值模式：捕获更多候选链接
/// let config = ExtractorConfig {
///     score_threshold: 0.5,
///     min_text_len: 4,
///     ..ExtractorConfig::default()
/// };
///
/// // 严格模式：只保留高置信度链接
/// let strict = ExtractorConfig {
///     score_threshold: 3.0,
///     cluster_min_size: 6,
///     ..ExtractorConfig::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    /// 最终判定为"文章链接"的分数阈值
    pub score_threshold: f32,
    /// 向上查找父节点计算路径签名的层数（越大越精细，但也越容易把
    /// 结构略有差异的同类项拆散）
    pub cluster_depth: usize,
    /// 一个聚类至少要有多少条链接，才被视为"列表结构"强信号
    pub cluster_min_size: usize,
    /// 锚文本短于此长度的链接会被判定为可能是导航/按钮而非标题
    pub min_text_len: usize,
    /// 是否叠加 UrlRule 的正则规则作为附加信号
    pub use_url_rule: bool,
    /// URL 路径关键词惩罚列表（如 /people/ /about/），可追加站点自定义路径
    pub non_article_paths: Vec<String>,
}

fn default_non_article_paths() -> Vec<String> {
    vec![
        "/people/", "/team/", "/about/", "/contact/",
        "/careers/", "/staff/", "/leadership/", "/board/",
        "/privacy", "/join", "/copyright", "/terms", "/subscribe",
        "/accessibility", "/press-inquiries", "/internships",
        "/jurisdiction", "/committee-", "/subcommittee-",
        "/chairman-", "/ranking-member", "/whistleblower",
        "/sitemap", "/faq", "/donate", "/support-",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            score_threshold: 1.5,
            cluster_depth: 4,
            cluster_min_size: 4,
            min_text_len: 8,
            use_url_rule: true,
            non_article_paths: default_non_article_paths(),
        }
    }
}

/// 基于 DOM 结构聚类 + 多信号打分的文章链接提取器。
///
/// 算法流程：
/// 1. 提取页面中所有 `<a[href]>` 元素
/// 2. 对每个链接计算 DOM 路径签名（`dom_path_signature`），按签名聚类
/// 3. 对每个候选链接计算 8 个信号的加权评分
/// 4. 同一 URL 保留最高分，过滤低于阈值的链接，按分数降序排列
///
/// # 示例
///
/// ```rust
/// use crawlkit_parser::{LinkExtractor, ExtractorConfig};
///
/// let html = r#"
/// <html><body>
///     <div class="list">
///         <a href="/2024/01/15/story-one">第一篇新闻标题</a>
///         <a href="/2024/01/16/story-two">第二篇新闻标题</a>
///         <a href="/2024/01/17/story-three">第三篇新闻标题</a>
///         <a href="/2024/01/18/story-four">第四篇新闻标题</a>
///     </div>
/// </body></html>
/// "#;
/// let extractor = LinkExtractor::new(ExtractorConfig::default());
/// let results = extractor.extract(html, "https://example.com/");
/// assert_eq!(results.len(), 4);
/// ```
pub struct LinkExtractor {
    config: ExtractorConfig,
    url_rule: UrlRule,
}

struct Candidate {
    url: String,
    text: String,
    path_sig: String,
    href_raw: String,
}

impl LinkExtractor {
    /// 创建一个新的提取器。
    ///
    /// 内部使用默认的 `UrlRule`，可通过 `with_url_rule` 覆盖。
    ///
    /// ```rust
    /// use crawlkit_parser::{LinkExtractor, ExtractorConfig};
    ///
    /// let extractor = LinkExtractor::new(ExtractorConfig::default());
    /// ```
    pub fn new(config: ExtractorConfig) -> Self {
        Self {
            config,
            url_rule: UrlRule::default(),
        }
    }

    /// 设置自定义 UrlRule，覆盖默认规则。
    ///
    /// ```rust
    /// use crawlkit_parser::{LinkExtractor, ExtractorConfig, UrlRule};
    ///
    /// let rule = UrlRule::default().with_include(r"/press/");
    /// let extractor = LinkExtractor::new(ExtractorConfig::default())
    ///     .with_url_rule(rule);
    /// ```
    pub fn with_url_rule(mut self, rule: UrlRule) -> Self {
        self.url_rule = rule;
        self
    }

    /// 从 HTML 中提取候选文章链接，按 score 从高到低排列。
    ///
    /// # 参数
    /// - `html`: 页面 HTML 源码
    /// - `base_url`: 页面 URL，用于相对路径解析和站外链接判定
    ///
    /// # 返回值
    /// 按评分降序排列的文章链接列表，已过滤低于 `score_threshold` 的链接。
    ///
    /// ```rust
    /// use crawlkit_parser::{LinkExtractor, ExtractorConfig};
    ///
    /// let html = r#"
    /// <ul>
    ///     <li><a href="/news/2025/01/01/story-a">2025年第一篇新闻</a></li>
    ///     <li><a href="/news/2025/01/02/story-b">2025年第二篇新闻</a></li>
    ///     <li><a href="/news/2025/01/03/story-c">2025年第三篇新闻</a></li>
    ///     <li><a href="/news/2025/01/04/story-d">2025年第四篇新闻</a></li>
    /// </ul>
    /// "#;
    /// let extractor = LinkExtractor::new(ExtractorConfig::default());
    /// let results = extractor.extract(html, "https://example.com/");
    /// assert!(results.len() >= 4);
    /// for link in &results {
    ///     println!("{} score={}", link.url, link.score);
    /// }
    /// ```
    pub fn extract(&self, html: &str, base_url: &str) -> Vec<ExtractedLink> {
        let document = Html::parse_document(html);
        let a_selector = Selector::parse("a[href]").expect("valid selector");
        let base = Url::parse(base_url).ok();
        let base_host = base.as_ref().and_then(|u| u.host_str()).unwrap_or("");

        let mut candidates: Vec<Candidate> = Vec::new();

        for a in document.select(&a_selector) {
            let href = match a.value().attr("href") {
                Some(h) => h.trim(),
                None => continue,
            };
            if href.is_empty() || href.starts_with('#') {
                continue;
            }
            if href.starts_with("javascript:") || href.starts_with("mailto:") || href.starts_with("tel:") {
                continue;
            }

            let resolved = resolve_url(&base, href);
            let text = clean_text(&a.text().collect::<Vec<_>>().join(" "));
            let path_sig = dom_path_signature(a, self.config.cluster_depth);

            candidates.push(Candidate {
                url: resolved,
                text,
                path_sig,
                href_raw: href.to_string(),
            });
        }

        let mut cluster_counts: HashMap<String, usize> = HashMap::new();
        for c in &candidates {
            *cluster_counts.entry(c.path_sig.clone()).or_insert(0) += 1;
        }

        let mut best_by_url: HashMap<String, ExtractedLink> = HashMap::new();

        for c in &candidates {
            let cluster_size = *cluster_counts.get(&c.path_sig).unwrap_or(&0);
            let score = self.score_candidate(c, cluster_size, base_host);

            best_by_url
                .entry(c.url.clone())
                .and_modify(|entry| {
                    if c.text.chars().count() > entry.text.chars().count() {
                        entry.text = c.text.clone();
                    }
                    entry.score = entry.score.max(score);
                    entry.cluster_size = entry.cluster_size.max(cluster_size);
                })
                .or_insert_with(|| ExtractedLink {
                    url: c.url.clone(),
                    text: c.text.clone(),
                    score,
                    cluster_size,
                });
        }

        let mut results: Vec<ExtractedLink> = best_by_url
            .into_values()
            .filter(|l| l.score >= self.config.score_threshold)
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// 对单个候选链接计算 8 个信号的加权评分。
    ///
    /// | # | 信号 | 权重 | 说明 |
    /// |---|------|------|------|
    /// | 1 | 锚文本长度 | +1.0 / +0.5 / -0.5 | >= min_text_len 加分；<= 120 额外加分；空文本扣分 |
    /// | 2 | 聚类大小 | +1.5 / +0.5 | >= cluster_min_size 强加分；>= 2 小幅加分 |
    /// | 3 | UrlRule 匹配 | +0.5 | 复用正则规则作为辅助信号 |
    /// | 4 | 路径深度 | +0.3 / -1.0 | 路径段 >= 2 加分，浅路径扣分 |
    /// | 5 | 导航词 | -1.5 | 匹配 NAV_WORDS 列表扣分 |
    /// | 6 | 分页/数字 | -2.0 / -1.5 | 含 page 参数或纯数字文本扣分 |
    /// | 7 | 外站链接 | -2.0 | 不同主机名的链接扣分 |
    /// | 8 | 非文章路径 | -2.0 | 匹配 non_article_paths 扣分 |
    fn score_candidate(&self, c: &Candidate, cluster_size: usize, base_host: &str) -> f32 {
        let mut score = 0.0f32;

        let text_len = c.text.chars().count();
        if text_len >= self.config.min_text_len {
            score += 1.0;
            if text_len <= 120 {
                score += 0.5;
            }
        } else if text_len == 0 {
            score -= 0.5;
        }

        if cluster_size >= self.config.cluster_min_size {
            score += 1.5;
        } else if cluster_size >= 2 {
            score += 0.5;
        }

        if self.config.use_url_rule && self.url_rule.is_article_url(&c.url) {
            score += 0.5;
        }

        let path_segments = c
            .url
            .split('/')
            .skip(3)
            .filter(|s| !s.is_empty())
            .count();
        if path_segments >= 2 {
            score += 0.3;
        } else {
            score -= 1.0;
        }

        let lower_text = c.text.to_lowercase();
        const NAV_WORDS: [&str; 12] = [
            "home", "login", "sign in", "sign up", "subscribe", "next", "prev",
            "previous", "more", "更多", "首页", "登录",
        ];
        if NAV_WORDS.iter().any(|w| lower_text == *w) {
            score -= 1.5;
        }

        if c.href_raw.contains("?page=") || c.href_raw.contains("&page=") {
            score -= 2.0;
        }
        if !c.text.trim().is_empty() && c.text.trim().chars().all(|c| c.is_ascii_digit()) {
            score -= 1.5;
        }

        if let Ok(url) = Url::parse(&c.url)
            && let Some(host) = url.host_str()
                && !host.is_empty() && host != base_host {
                    score -= 2.0;
                }

        if self.config.non_article_paths.iter().any(|p| c.url.contains(p)) {
            score -= 2.0;
        }

        score
    }
}

fn resolve_url(base: &Option<Url>, href: &str) -> String {
    match base {
        Some(b) => b
            .join(href).map_or_else(|_| href.to_string(), |u| u.to_string()),
        None => href.to_string(),
    }
}

fn clean_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn dom_path_signature(a: ElementRef, depth: usize) -> String {
    let mut parts = Vec::new();
    let mut current = a.parent();
    let mut d = 0;

    while let Some(node) = current {
        if d >= depth {
            break;
        }
        if let Some(el) = ElementRef::wrap(node) {
            let tag = el.value().name();
            if tag == "html" || tag == "body" {
                break;
            }
            let mut classes: Vec<&str> = el.value().classes().collect();
            classes.sort_unstable();
            let class_part = if classes.is_empty() {
                String::new()
            } else {
                format!(".{}", classes.join("."))
            };
            parts.push(format!("{tag}{class_part}"));
        }
        current = node.parent();
        d += 1;
    }

    parts.reverse();
    parts.join(">")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracts_repeated_list_items_over_scattered_nav_links() {
        let html = r#"
        <html><body>
            <nav>
                <a href="/">首页</a>
                <a href="/login">登录</a>
            </nav>
            <div class="news-list">
                <ul>
                    <li><a href="/2024/01/15/story-one">这是第一条新闻标题</a></li>
                    <li><a href="/2024/01/16/story-two">这是第二条新闻标题</a></li>
                    <li><a href="/2024/01/17/story-three">这是第三条新闻标题</a></li>
                    <li><a href="/2024/01/18/story-four">这是第四条新闻标题</a></li>
                    <li><a href="/2024/01/19/story-five">这是第五条新闻标题</a></li>
                </ul>
            </div>
        </body></html>
        "#;

        let extractor = LinkExtractor::new(ExtractorConfig::default());
        let results = extractor.extract(html, "https://example.com/");
        let urls: Vec<&str> = results.iter().map(|l| l.url.as_str()).collect();

        assert!(urls.iter().any(|u| u.contains("story-one")));
        assert!(urls.iter().any(|u| u.contains("story-five")));
        assert!(!urls.iter().any(|u| u.ends_with("/login")));
    }

    #[test]
    fn test_short_nav_text_is_penalized() {
        let html = r#"
        <html><body>
            <a href="/next">Next</a>
            <a href="/article/full-title-of-a-real-article">完整的一篇真实文章标题在这里</a>
        </body></html>
        "#;
        let extractor = LinkExtractor::new(ExtractorConfig::default());
        let results = extractor.extract(html, "https://example.com/");
        assert!(results[0].url.contains("full-title-of-a-real-article"));
    }

    #[test]
    fn test_relative_url_resolution() {
        let html = r#"<html><body><a href="/news/foo">新闻标题足够长这样才算数</a></body></html>"#;
        let extractor = LinkExtractor::new(ExtractorConfig::default());
        let results = extractor.extract(html, "https://example.com/section/");
        assert_eq!(results[0].url, "https://example.com/news/foo");
    }

    #[test]
    fn test_slug_only_site_without_regex_hints() {
        let html = r#"
        <html><body>
            <div class="views-row"><a href="/health-policy/making-a-deposit/">Making a Deposit in Health Policy</a></div>
            <div class="views-row"><a href="/economics/inflation-outlook/">The Inflation Outlook</a></div>
            <div class="views-row"><a href="/foreign-policy/china-strategy/">Rethinking China Strategy</a></div>
            <div class="views-row"><a href="/education/school-choice/">The Case for School Choice</a></div>
        </body></html>
        "#;
        let extractor = LinkExtractor::new(ExtractorConfig {
            use_url_rule: false,
            ..ExtractorConfig::default()
        });
        let results = extractor.extract(html, "https://www.aei.org/");
        assert!(results.iter().any(|l| l.url.contains("making-a-deposit")));
        assert!(results.iter().any(|l| l.url.contains("china-strategy")));
    }

    #[test]
    fn test_with_url_rule_boosts_score() {
        let html = r#"<html><body><a href="/2024/01/15/some-story">一篇独立的新闻文章标题</a></body></html>"#;
        let with_rule = LinkExtractor::new(ExtractorConfig::default());
        let without_rule = LinkExtractor::new(ExtractorConfig {
            use_url_rule: false,
            ..ExtractorConfig::default()
        });
        let r1 = with_rule.extract(html, "https://example.com/");
        let r2 = without_rule.extract(html, "https://example.com/");
        assert!(r1[0].score > r2[0].score);
    }
}
