//! CSS 选择器工具模块
//!
//! 提供预编译的选择器缓存、动态选择器创建、选择器构建函数等功能。
//! 基于 scraper crate 实现，适配自 halldyll-parser。

use lazy_static::lazy_static;
use scraper::Selector;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::types::{ParserError, ParserResult};

/// 预编译的 CSS 选择器集合
///
/// 通过 `lazy_static` 的 `SELECTORS` 全局实例访问。
/// 所有选择器在首次访问时编译一次，后续复用。
pub struct CachedSelectors {
    /// `title` 元素选择器
    pub title: Selector,
    /// `meta` 元素选择器
    pub meta: Selector,
    /// `link` 元素选择器
    pub link: Selector,
    /// `base` 元素选择器
    pub base: Selector,
    /// `html` 元素选择器
    pub html: Selector,
    /// `body` 元素选择器
    pub body: Selector,
    /// `article` 元素选择器
    pub article: Selector,
    /// `main` 元素选择器
    pub main: Selector,
    /// `[role=main]` 选择器
    pub main_role: Selector,
    /// 标题元素集合选择器：h1, h2, h3, h4, h5, h6
    pub headings: Selector,
    /// `p` 元素选择器
    pub p: Selector,
    /// `blockquote` 元素选择器
    pub blockquote: Selector,
    /// `pre` 元素选择器
    pub pre: Selector,
    /// `pre code` 后代选择器（代码块）
    pub pre_code: Selector,
    /// `code` 元素选择器
    pub code: Selector,
    /// `ul` 元素选择器
    pub ul: Selector,
    /// `ol` 元素选择器
    pub ol: Selector,
    /// `li` 元素选择器
    pub li: Selector,
    /// `dl` 元素选择器
    pub dl: Selector,
    /// `dt` 元素选择器
    pub dt: Selector,
    /// `dd` 元素选择器
    pub dd: Selector,
    /// `table` 元素选择器
    pub table: Selector,
    /// `thead` 元素选择器
    pub thead: Selector,
    /// `tbody` 元素选择器
    pub tbody: Selector,
    /// `tfoot` 元素选择器
    pub tfoot: Selector,
    /// `tr` 元素选择器
    pub tr: Selector,
    /// `th` 元素选择器
    pub th: Selector,
    /// `td` 元素选择器
    pub td: Selector,
    /// `caption` 元素选择器
    pub caption: Selector,
    /// `a` 元素选择器
    pub a: Selector,
    /// `img` 元素选择器
    pub img: Selector,
    /// `picture` 元素选择器
    pub picture: Selector,
    /// `source` 元素选择器
    pub source: Selector,
    /// `figure` 元素选择器
    pub figure: Selector,
    /// `figcaption` 元素选择器
    pub figcaption: Selector,
    /// `script` 元素选择器
    pub script: Selector,
    /// `style` 元素选择器
    pub style: Selector,
    /// `noscript` 元素选择器
    pub noscript: Selector,
    /// `nav` 元素选择器
    pub nav: Selector,
    /// `header` 元素选择器
    pub header: Selector,
    /// `footer` 元素选择器
    pub footer: Selector,
    /// `aside` 元素选择器
    pub aside: Selector,
    /// `script[type="application/ld+json"]` 选择器（JSON-LD 结构化数据）
    pub json_ld: Selector,
    /// `[itemscope][itemtype]` 选择器（Microdata 结构化数据）
    pub microdata: Selector,
}

lazy_static! {
    /// 预编译 CSS 选择器全局实例
    ///
    /// 所有内置选择器在此处一次性编译，避免重复解析。
    pub static ref SELECTORS: CachedSelectors = CachedSelectors {
        title: sel("title"),
        meta: sel("meta"),
        link: sel("link"),
        base: sel("base"),
        html: sel("html"),
        body: sel("body"),
        article: sel("article"),
        main: sel("main"),
        main_role: sel("[role=main]"),
        headings: sel("h1, h2, h3, h4, h5, h6"),
        p: sel("p"),
        blockquote: sel("blockquote"),
        pre: sel("pre"),
        pre_code: sel("pre code"),
        code: sel("code"),
        ul: sel("ul"),
        ol: sel("ol"),
        li: sel("li"),
        dl: sel("dl"),
        dt: sel("dt"),
        dd: sel("dd"),
        table: sel("table"),
        thead: sel("thead"),
        tbody: sel("tbody"),
        tfoot: sel("tfoot"),
        tr: sel("tr"),
        th: sel("th"),
        td: sel("td"),
        caption: sel("caption"),
        a: sel("a"),
        img: sel("img"),
        picture: sel("picture"),
        source: sel("source"),
        figure: sel("figure"),
        figcaption: sel("figcaption"),
        script: sel("script"),
        style: sel("style"),
        noscript: sel("noscript"),
        nav: sel("nav"),
        header: sel("header"),
        footer: sel("footer"),
        aside: sel("aside"),
        json_ld: sel(r#"script[type="application/ld+json"]"#),
        microdata: sel("[itemscope][itemtype]"),
    };

    /// 动态选择器缓存
    ///
    /// 用于缓存运行时动态创建的选择器，避免重复解析。
    /// 使用 `RwLock` 保证线程安全。
    static ref SELECTOR_CACHE: RwLock<HashMap<String, Selector>> = RwLock::new(HashMap::new());
}

/// 编译 CSS 选择器，失败时 panic
///
/// 仅用于预编译已知合法的选择器。
fn sel(css: &str) -> Selector {
    Selector::parse(css).unwrap_or_else(|e| {
        panic!("无效的 CSS 选择器 '{css}': {e}")
    })
}

/// 获取或创建动态选择器（带缓存）
///
/// 先从缓存查找，如果不存在则编译并存入缓存。
/// 如果选择器字符串无效，返回 `ParserError::SelectorError`。
pub fn get_or_create_selector(css: &str) -> ParserResult<Selector> {
    {
        let cache = SELECTOR_CACHE.read().map_err(|e| {
            ParserError::ConfigError(format!("选择器缓存读取锁失效: {e}"))
        })?;
        if let Some(sel) = cache.get(css) {
            return Ok(sel.clone());
        }
    }

    let selector = Selector::parse(css).map_err(|e| {
        ParserError::SelectorError(format!("无法解析选择器 '{css}': {e}"))
    })?;

    let mut cache = SELECTOR_CACHE.write().map_err(|e| {
        ParserError::ConfigError(format!("选择器缓存写入锁失效: {e}"))
    })?;
    cache.insert(css.to_string(), selector.clone());
    Ok(selector)
}

/// 解析 CSS 选择器，失败时 panic
///
/// 不经过缓存，每次都重新解析。
/// 适用于脚本阶段就知道合法的选择器。
pub fn parse_selector(css: &str) -> Selector {
    Selector::parse(css).unwrap_or_else(|e| {
        panic!("无效的 CSS 选择器 '{css}': {e}")
    })
}

/// 尝试解析 CSS 选择器，失败时返回 `None`
///
/// 不经过缓存，每次都重新解析。
pub fn try_parse_selector(css: &str) -> Option<Selector> {
    Selector::parse(css).ok()
}

/// 返回匹配所有标题元素（h1-h6）的 CSS 选择器字符串
///
/// 等同于 `"h1, h2, h3, h4, h5, h6"`。
pub fn heading_selector() -> &'static str {
    "h1, h2, h3, h4, h5, h6"
}

// ─── 常用选择器集合 ─────────────────────────────────────────

/// 内容区域选择器列表
///
/// 用于定位页面的主要内容区域。
pub const CONTENT_SELECTORS: &[&str] = &[
    "article",
    "main",
    "[role=main]",
    ".content",
    ".post-content",
    ".entry-content",
    ".article-content",
    ".post-body",
    ".article-body",
    "#content",
    "#main-content",
    ".main-content",
];

lazy_static! {
    /// 样板（非内容）区域选择器集合
    ///
    /// 用于移除页面中的导航、广告、侧边栏等非核心内容。
    /// 使用 `HashSet` 以便快速查找和克隆。
    pub static ref BOILERPLATE_SELECTORS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("script");
        set.insert("style");
        set.insert("noscript");
        set.insert("nav");
        set.insert("header");
        set.insert("footer");
        set.insert("aside");
        set.insert(".sidebar");
        set.insert(".advertisement");
        set.insert(".ad");
        set.insert(".ads");
        set.insert(".menu");
        set.insert(".navigation");
        set.insert(".nav");
        set.insert(".footer");
        set.insert(".header");
        set.insert("[role=navigation]");
        set.insert("[role=banner]");
        set.insert("[role=contentinfo]");
        set
    };
}

/// 内联元素名列表
///
/// 这些元素通常不会产生换行，在文本提取时应作为内联处理。
pub const INLINE_ELEMENTS: &[&str] = &[
    "a", "abbr", "acronym", "b", "bdo", "big", "br", "button", "cite",
    "code", "dfn", "em", "i", "img", "input", "kbd", "label", "map",
    "object", "q", "samp", "script", "select", "small", "span", "strong",
    "sub", "sup", "textarea", "time", "tt", "var",
];

/// 块级元素名列表
///
/// 这些元素通常会换行并占据整行空间，在文本提取时应作为块级处理。
pub const BLOCK_ELEMENTS: &[&str] = &[
    "address", "article", "aside", "blockquote", "canvas", "dd", "div",
    "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form",
    "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup", "hr", "li",
    "main", "nav", "noscript", "ol", "output", "p", "pre", "section",
    "table", "tfoot", "ul", "video",
];

// ─── 选择器构建函数 ─────────────────────────────────────────

/// 构建属性选择器：`[attr]`
///
/// 匹配所有包含指定属性的元素，无论属性值是什么。
pub fn attr_selector(attr: &str) -> String {
    format!("[{attr}]")
}

/// 构建属性包含选择器：`[attr*=value]`
///
/// 匹配属性值中包含指定子串的元素。
pub fn attr_contains_selector(attr: &str, value: &str) -> String {
    format!(r#"[{attr}*="{value}"]"#)
}

/// 构建属性前缀选择器：`[attr^=value]`
///
/// 匹配属性值以指定字符串开头的元素。
pub fn attr_starts_with_selector(attr: &str, value: &str) -> String {
    format!(r#"[{attr}^="{value}"]"#)
}

/// 构建 class 选择器：`.class`
///
/// 匹配所有包含指定 class 的元素。
pub fn class_selector(class: &str) -> String {
    format!(".{class}")
}

/// 构建 ID 选择器：`#id`
pub fn id_selector(id: &str) -> String {
    format!("#{id}")
}

/// 构建后代选择器：`ancestor descendant`
///
/// 匹配 ancestor 元素下的所有 descendant 元素。
pub fn descendant_selector(ancestor: &str, descendant: &str) -> String {
    format!("{ancestor} {descendant}")
}

/// 构建子选择器：`parent > child`
///
/// 仅匹配 parent 的直接子元素 child。
pub fn child_selector(parent: &str, child: &str) -> String {
    format!("{parent} > {child}")
}

/// 构建多重选择器：`sel1, sel2, sel3, ...`
///
/// 将多个选择器合并为一个逗号分隔的组合选择器。
pub fn multi_selector(selectors: &[&str]) -> String {
    selectors.join(", ")
}

// ─── Meta 标签选择器 ────────────────────────────────────────

/// 构建 `meta[name="..."]` 选择器
///
/// 用于匹配特定 name 属性的 `<meta>` 标签。
pub fn meta_name_selector(name: &str) -> String {
    format!(r#"meta[name="{name}"]"#)
}

/// 构建 `meta[property="..."]` 选择器
///
/// 用于匹配特定 property 属性的 `<meta>` 标签（Open Graph 等）。
pub fn meta_property_selector(property: &str) -> String {
    format!(r#"meta[property="{property}"]"#)
}

/// 构建 `link[rel="..."]` 选择器
///
/// 用于匹配特定 rel 属性的 `<link>` 标签。
pub fn link_rel_selector(rel: &str) -> String {
    format!(r#"link[rel="{rel}"]"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    /// 辅助：计算 HTML 片段中匹配选择器的元素数量
    fn count_matches(html: &str, selector: &str) -> usize {
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse(selector).unwrap();
        doc.select(&sel).count()
    }

    /// 辅助：在文档模式下计算匹配数
    fn count_matches_doc(html: &str, selector: &Selector) -> usize {
        let doc = Html::parse_document(html);
        doc.select(selector).count()
    }

    // ─── 预编译选择器测试 ──────────────────────────────────

    #[test]
    fn test_precompiled_selectors_match_expected_elements() {
        let html = r##"
            <html><head><title>测试</title></head><body>
                <h1>标题1</h1><h2>标题2</h2><h6>标题6</h6>
                <p>段落</p>
                <a href="#">链接</a>
                <img src="test.jpg" />
                <ul><li>列表项</li></ul>
                <table><thead><tr><th>表头</th></tr></thead>
                       <tbody><tr><td>单元格</td></tr></tbody>
                       <tfoot><tr><td>页脚</td></tr></tfoot>
                       <caption>表格标题</caption></table>
                <article>文章</article>
                <main>主体</main>
                <div role="main">角色主体</div>
                <nav>导航</nav>
                <footer>页脚</footer>
                <header>页眉</header>
                <aside>侧边栏</aside>
                <blockquote>引用</blockquote>
                <pre><code>代码</code></pre>
                <code>内联代码</code>
                <dl><dt>术语</dt><dd>定义</dd></dl>
                <ol><li>有序项</li></ol>
                <picture><source srcset="img.webp" /><img src="img.jpg" /></picture>
                <figure><figcaption>图注</figcaption></figure>
                <script>js</script>
                <style>css</style>
                <noscript>备选</noscript>
                <script type="application/ld+json">{}</script>
                <div itemscope itemtype="https://example.org/Article">微数据</div>
            </body></html>
        "##;

        assert_eq!(count_matches_doc(html, &SELECTORS.title), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.meta), 0);
        assert_eq!(count_matches_doc(html, &SELECTORS.html), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.body), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.headings), 3);
        assert_eq!(count_matches_doc(html, &SELECTORS.p), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.a), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.img), 2);
        assert_eq!(count_matches_doc(html, &SELECTORS.ul), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.ol), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.li), 2);
        assert_eq!(count_matches_doc(html, &SELECTORS.table), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.thead), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.tbody), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.tfoot), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.tr), 3);
        assert_eq!(count_matches_doc(html, &SELECTORS.th), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.td), 2);
        assert_eq!(count_matches_doc(html, &SELECTORS.caption), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.article), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.main), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.main_role), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.nav), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.footer), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.header), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.aside), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.blockquote), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.pre), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.pre_code), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.code), 2);
        assert_eq!(count_matches_doc(html, &SELECTORS.dl), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.dt), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.dd), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.picture), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.source), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.figure), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.figcaption), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.script), 2);
        assert_eq!(count_matches_doc(html, &SELECTORS.style), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.noscript), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.json_ld), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.microdata), 1);
    }

    #[test]
    fn test_precompiled_link_base_selectors() {
        let html = r#"
            <html><head>
                <base href="https://example.com/" />
                <link rel="stylesheet" href="style.css" />
            </head></html>
        "#;
        assert_eq!(count_matches_doc(html, &SELECTORS.link), 1);
        assert_eq!(count_matches_doc(html, &SELECTORS.base), 1);
    }

    // ─── 动态选择器缓存测试 ────────────────────────────────

    #[test]
    fn test_get_or_create_selector_creates_and_caches() {
        let sel1 = get_or_create_selector("div.content").unwrap();
        let sel2 = get_or_create_selector("div.content").unwrap();

        // 验证两个选择器行为一致
        let html = r#"<div class="content">内容</div>"#;
        let doc = Html::parse_fragment(html);
        assert_eq!(doc.select(&sel1).count(), doc.select(&sel2).count());
        assert_eq!(doc.select(&sel1).count(), 1);
    }

    #[test]
    fn test_get_or_create_selector_returns_error_for_invalid() {
        let result = get_or_create_selector("!!!invalid");
        assert!(result.is_err());
        match result {
            Err(ParserError::SelectorError(_)) => {}
            _ => panic!("期望 SelectorError 变体"),
        }
    }

    #[test]
    fn test_try_parse_selector_returns_none_for_invalid() {
        let result = try_parse_selector("!!!invalid");
        assert!(result.is_none());
    }

    #[test]
    fn test_try_parse_selector_returns_some_for_valid() {
        let result = try_parse_selector("div");
        assert!(result.is_some());
    }

    // ─── heading_selector 测试 ─────────────────────────────

    #[test]
    fn test_heading_selector_format() {
        assert_eq!(heading_selector(), "h1, h2, h3, h4, h5, h6");
    }

    #[test]
    fn test_heading_selector_matches_all_levels() {
        let html = r"
            <h1>一级</h1>
            <h2>二级</h2>
            <h3>三级</h3>
            <h4>四级</h4>
            <h5>五级</h5>
            <h6>六级</h6>
        ";
        assert_eq!(count_matches(html, heading_selector()), 6);
    }

    // ─── 选择器常量测试 ────────────────────────────────────

    #[test]
    fn test_content_selectors_contains_core_selectors() {
        assert!(CONTENT_SELECTORS.contains(&"article"));
        assert!(CONTENT_SELECTORS.contains(&"main"));
        assert!(CONTENT_SELECTORS.contains(&"[role=main]"));
        assert!(!CONTENT_SELECTORS.is_empty());
    }

    #[test]
    fn test_boilerplate_selectors_contains_core_selectors() {
        assert!(BOILERPLATE_SELECTORS.contains(&"nav"));
        assert!(BOILERPLATE_SELECTORS.contains(&"footer"));
        assert!(BOILERPLATE_SELECTORS.contains(&"aside"));
        assert!(!BOILERPLATE_SELECTORS.is_empty());
    }

    #[test]
    fn test_inline_and_block_elements_are_disjoint() {
        for elem in INLINE_ELEMENTS {
            assert!(
                !BLOCK_ELEMENTS.contains(elem),
                "元素 {elem} 同时出现在 INLINE 和 BLOCK 中"
            );
        }
        assert!(INLINE_ELEMENTS.contains(&"span"));
        assert!(BLOCK_ELEMENTS.contains(&"div"));
    }

    // ─── 选择器构建函数测试 ──────────────────────────────────

    #[test]
    fn test_attr_selector_builders() {
        assert_eq!(attr_selector("href"), "[href]");
        assert_eq!(attr_contains_selector("class", "active"), r#"[class*="active"]"#);
        assert_eq!(attr_starts_with_selector("href", "/"), r#"[href^="/"]"#);
    }

    #[test]
    fn test_class_and_id_selector_builders() {
        assert_eq!(class_selector("container"), ".container");
        assert_eq!(id_selector("main"), "#main");
    }

    #[test]
    fn test_combinator_selector_builders() {
        assert_eq!(descendant_selector("div", "p"), "div p");
        assert_eq!(child_selector("ul", "li"), "ul > li");
    }

    #[test]
    fn test_multi_selector_builder() {
        assert_eq!(multi_selector(&["h1", "h2", "h3"]), "h1, h2, h3");
        assert_eq!(multi_selector(&[]), "");
    }

    // ─── Meta 选择器测试 ──────────────────────────────────────

    #[test]
    fn test_meta_name_selector_matches_meta_name() {
        let sel = meta_name_selector("description");
        assert_eq!(sel, r#"meta[name="description"]"#);
        assert_eq!(count_matches(r#"<meta name="description" content="d">"#, &sel), 1);
        assert_eq!(count_matches(r#"<meta name="keywords" content="k">"#, &sel), 0);
    }

    #[test]
    fn test_meta_property_selector_matches_meta_property() {
        let sel = meta_property_selector("og:title");
        assert_eq!(sel, r#"meta[property="og:title"]"#);
        assert_eq!(count_matches(r#"<meta property="og:title" content="标题">"#, &sel), 1);
    }

    #[test]
    fn test_link_rel_selector_matches_link_rel() {
        let sel = link_rel_selector("canonical");
        assert_eq!(sel, r#"link[rel="canonical"]"#);
        assert_eq!(count_matches(r#"<link rel="canonical" href="https://ex.com">"#, &sel), 1);
        assert_eq!(count_matches(r#"<link rel="stylesheet" href="s.css">"#, &sel), 0);
    }

    // ─── 集成测试 ──────────────────────────────────────────

    #[test]
    fn test_builder_functions_compose_correctly() {
        let composed = descendant_selector("article", &class_selector("post-content"));
        assert_eq!(composed, "article .post-content");

        let html = r#"<article><div class="post-content"><p>正文</p></div></article>"#;
        assert_eq!(count_matches(html, &composed), 1);
    }

    #[test]
    fn test_cached_selector_matches_raw_selector() {
        let html = r#"<div id="unique-id">唯一元素</div>"#;

        let raw = Selector::parse("div#unique-id").unwrap();
        let cached = get_or_create_selector("div#unique-id").unwrap();

        let doc = Html::parse_fragment(html);
        assert_eq!(doc.select(&raw).count(), doc.select(&cached).count());
    }
}
