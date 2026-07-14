//! 结构化内容提取模块
//!
//! 提供从 HTML 中提取标题、段落、列表、表格、代码块、引用、图片等
//! 结构化内容的功能。适配自 halldyll-parser 的内容提取实现。

use scraper::{Html, ElementRef};
use url::Url;

use crate::selector::try_parse_selector;
use crate::types::{
    Heading, Image, ImageLoading, ListContent, ListType, ListItem,
    TableContent, TableRow, TableCell, CodeBlock, Quote,
    ParserConfig,
};

// ============================================================================
// 大纲条目
// ============================================================================

/// 大纲条目，用于构建页面标题的层级树结构
#[derive(Debug, Clone)]
pub struct OutlineItem {
    /// 标题内容
    pub heading: Heading,
    /// 子标题列表
    pub children: Vec<OutlineItem>,
    /// 当前深度（从 0 开始）
    pub depth: usize,
}

impl OutlineItem {
    pub fn new(heading: Heading) -> Self {
        Self {
            heading,
            children: Vec::new(),
            depth: 0,
        }
    }
}

// ============================================================================
// 标题提取
// ============================================================================

/// 从 HTML 文档中提取所有标题（h1 - h6）
///
/// 返回按文档顺序排列的标题列表，包含层级、文本、ID 和 class 信息。
pub fn extract_headings(document: &Html) -> Vec<Heading> {
    let mut headings = Vec::new();

    for level in 1..=6u8 {
        let selector_str = format!("h{level}");
        let selector = match try_parse_selector(&selector_str) {
            Some(s) => s,
            None => continue,
        };
        for element in document.select(&selector) {
            let text: String = element
                .text()
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();

            if text.is_empty() {
                continue;
            }

            let id = element.value().attr("id").map(ToString::to_string);
            let classes: Vec<String> = element
                .value()
                .classes()
                .map(std::string::ToString::to_string)
                .collect();

            headings.push(Heading {
                level,
                text,
                id,
                classes,
            });
        }
    }

    headings
}

/// 获取页面的主标题（第一个 h1，回退到第一个 h2，再回退到 <title>）
pub fn get_main_heading(document: &Html) -> Option<Heading> {
    // 尝试 h1
    if let Some(sel) = try_parse_selector("h1")
        && let Some(el) = document.select(&sel).next() {
            let text: String = el
                .text()
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();
            if !text.is_empty() {
                let id = el.value().attr("id").map(ToString::to_string);
                let classes: Vec<String> = el
                    .value()
                    .classes()
                    .map(std::string::ToString::to_string)
                    .collect();
                return Some(Heading {
                    level: 1,
                    text,
                    id,
                    classes,
                });
            }
        }

    // 回退到 h2
    if let Some(sel) = try_parse_selector("h2")
        && let Some(el) = document.select(&sel).next() {
            let text: String = el
                .text()
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();
            if !text.is_empty() {
                let id = el.value().attr("id").map(ToString::to_string);
                let classes: Vec<String> = el
                    .value()
                    .classes()
                    .map(std::string::ToString::to_string)
                    .collect();
                return Some(Heading {
                    level: 2,
                    text,
                    id,
                    classes,
                });
            }
        }

    None
}

/// 将标题列表构建为层级大纲树
///
/// 根据标题的 level 字段组装为嵌套的 `OutlineItem` 树结构。
pub fn build_outline(headings: &[Heading]) -> Vec<OutlineItem> {
    fn build(items: &[Heading], min_level: u8, depth: usize) -> (Vec<OutlineItem>, usize) {
        let mut result = Vec::new();
        let mut i = 0;
        while i < items.len() {
            let h = &items[i];
            if h.level < min_level {
                break;
            }
            let mut item = OutlineItem::new(h.clone());
            item.depth = depth;
            let (children, consumed) = build(&items[i + 1..], h.level + 1, depth + 1);
            item.children = children;
            i += 1 + consumed;
            result.push(item);
        }
        (result, i)
    }

    let (outline, _) = build(headings, 1, 0);
    outline
}

// ============================================================================
// 段落提取
// ============================================================================

/// 从 HTML 文档中提取所有段落文本
///
/// 匹配 `<p>` 标签，过滤掉空段落和过短的段落。
pub fn extract_paragraphs(document: &Html, min_length: usize) -> Vec<String> {
    let selector = match try_parse_selector("p") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut paragraphs = Vec::new();

    for element in document.select(&selector) {
        let text: String = element
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();

        if text.len() >= min_length {
            paragraphs.push(text);
        }
    }

    paragraphs
}

// ============================================================================
// 列表提取
// ============================================================================

/// 从 HTML 文档中提取所有列表
///
/// 包括有序列表 `<ol>`、无序列表 `<ul>` 和定义列表 `<dl>`。
pub fn extract_lists(document: &Html, config: &ParserConfig) -> Vec<ListContent> {
    let mut lists = Vec::new();

    // 提取有序和无序列表
    for list_tag in &["ul", "ol"] {
        let selector = match try_parse_selector(list_tag) {
            Some(s) => s,
            None => continue,
        };
        for element in document.select(&selector) {
            // 跳过嵌套在列表项内的列表（由 extract_list_item 处理）
            let parent = element.parent();
            if let Some(parent_id) = parent
                && let Some(parent_ref) = ElementRef::wrap(parent_id)
                    && parent_ref.value().name() == "li" {
                        continue;
                    }
            let list = extract_list(&element, config);
            if !list.is_empty() {
                lists.push(list);
            }
        }
    }

    // 提取定义列表
    if let Some(sel) = try_parse_selector("dl") {
        for element in document.select(&sel) {
            let list = extract_definition_list(&element, config);
            if !list.is_empty() {
                lists.push(list);
            }
        }
    }

    lists
}

/// 从元素中提取单个列表（ol/ul）
pub fn extract_list(element: &ElementRef, _config: &ParserConfig) -> ListContent {
    let tag_name = element.value().name();
    let list_type = match tag_name {
        "ol" => ListType::Ordered,
        _ => ListType::Unordered,
    };

    let mut list = ListContent::new(list_type);

    for child in element.children() {
        let child_ref = match ElementRef::wrap(child) {
            Some(r) => r,
            None => continue,
        };
        if child_ref.value().name() != "li" {
            continue;
        }
        if let Some(item) = extract_list_item(&child_ref) {
            list.add_item(item);
        }
    }

    list
}

/// 从 li 元素中提取列表项，包括可能的内嵌列表
pub fn extract_list_item(element: &ElementRef) -> Option<ListItem> {
    let mut text_parts = Vec::new();
    let mut nested_list: Option<ListContent> = None;
    let mut has_text = false;

    for child in element.children() {
        let child_ref = match ElementRef::wrap(child) {
            Some(r) => r,
            None => {
                // 文本节点
                if let scraper::Node::Text(t) = child.value() {
                    let t = t.text.trim();
                    if !t.is_empty() {
                        text_parts.push(t.to_string());
                        has_text = true;
                    }
                }
                continue;
            }
        };

        let name = child_ref.value().name();
        match name {
            "ul" | "ol" => {
                let lt = if name == "ol" {
                    ListType::Ordered
                } else {
                    ListType::Unordered
                };
                let mut nl = ListContent::new(lt);
                for subchild in child_ref.children() {
                    if let Some(sub_ref) = ElementRef::wrap(subchild)
                        && sub_ref.value().name() == "li"
                            && let Some(item) = extract_list_item(&sub_ref) {
                                nl.add_item(item);
                            }
                }
                if !nl.is_empty() {
                    nested_list = Some(nl);
                }
            }
            _ => {
                let t: String = child_ref
                    .text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string();
                if !t.is_empty() {
                    text_parts.push(t);
                    has_text = true;
                }
            }
        }
    }

    if !has_text {
        return None;
    }

    let text = text_parts.join(" ");

    if let Some(nl) = nested_list {
        Some(ListItem::with_nested(text, nl))
    } else {
        Some(ListItem::new(text))
    }
}

/// 从 dl 元素中提取定义列表
pub fn extract_definition_list(element: &ElementRef, _config: &ParserConfig) -> ListContent {
    let mut list = ListContent::new(ListType::Definition);

    let mut current_term: Option<String> = None;

    for child in element.children() {
        let child_ref = match ElementRef::wrap(child) {
            Some(r) => r,
            None => continue,
        };

        let name = child_ref.value().name();
        match name {
            "dt" => {
                let text: String = child_ref
                    .text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    current_term = Some(text);
                }
            }
            "dd" => {
                let text: String = child_ref
                    .text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    let item_text = if let Some(ref term) = current_term {
                        format!("{term}: {text}")
                    } else {
                        text
                    };
                    list.add_item(ListItem::new(item_text));
                }
            }
            _ => {}
        }
    }

    list
}

// ============================================================================
// 表格提取
// ============================================================================

/// 从 HTML 文档中提取所有表格
pub fn extract_tables(document: &Html, config: &ParserConfig) -> Vec<TableContent> {
    let selector = match try_parse_selector("table") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut tables = Vec::new();

    for element in document.select(&selector) {
        let table = extract_table(&element, config);
        if !table.is_empty() {
            tables.push(table);
        }
    }

    tables
}

/// 从 table 元素中提取单个表格
pub fn extract_table(element: &ElementRef, _config: &ParserConfig) -> TableContent {
    let mut table = TableContent::new();

    // 提取表格标题
    if let Some(caption_sel) = try_parse_selector("caption")
        && let Some(caption_el) = element.select(&caption_sel).next() {
            let caption: String = caption_el
                .text()
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();
            if !caption.is_empty() {
                table.caption = Some(caption);
            }
        }

    // 提取表头（thead 中的行）
    if let Some(thead_sel) = try_parse_selector("thead") {
        let mut header_rows = Vec::new();
        if let Some(thead_el) = element.select(&thead_sel).next() {
            for row_el in thead_el.child_elements() {
                let row = extract_table_row(&row_el);
                if !row.cells.is_empty() {
                    header_rows.push(row);
                }
            }
        }
        table.headers = header_rows;
    }

    // 提取表体行（tbody 或直接子级 tr）
    let mut rows = Vec::new();
    if let Some(tbody_sel) = try_parse_selector("tbody") {
        if let Some(tbody_el) = element.select(&tbody_sel).next() {
            for row_el in tbody_el.child_elements() {
                if row_el.value().name() == "tr" {
                    let row = extract_table_row(&row_el);
                    if !row.cells.is_empty() {
                        rows.push(row);
                    }
                }
            }
        } else {
            // 没有 tbody，直接从 table 下找 tr
            for child in element.children() {
                if let Some(row_ref) = ElementRef::wrap(child)
                    && row_ref.value().name() == "tr" {
                        let row = extract_table_row(&row_ref);
                        if !row.cells.is_empty() {
                            rows.push(row);
                        }
                    }
            }
        }
    }

    table.rows = rows;

    // 计算列数
    let max_cols = table
        .headers
        .iter()
        .chain(table.rows.iter())
        .map(|r| r.cells.len())
        .max()
        .unwrap_or(0);
    table.column_count = max_cols;

    table
}

/// 从 tr 元素中提取表格行
pub fn extract_table_row(element: &ElementRef) -> TableRow {
    let mut cells = Vec::new();

    for child in element.children() {
        let child_ref = match ElementRef::wrap(child) {
            Some(r) => r,
            None => continue,
        };

        let name = child_ref.value().name();
        if name != "td" && name != "th" {
            continue;
        }

        let cell = extract_table_cell(&child_ref);
        cells.push(cell);
    }

    TableRow::new(cells)
}

/// 从 td 或 th 元素中提取表格单元格
pub fn extract_table_cell(element: &ElementRef) -> TableCell {
    let is_header = element.value().name() == "th";

    let content: String = element
        .text()
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();

    let colspan = element
        .value()
        .attr("colspan")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1);

    let rowspan = element
        .value()
        .attr("rowspan")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1);

    TableCell {
        content,
        is_header,
        colspan,
        rowspan,
    }
}

// ============================================================================
// 代码块提取
// ============================================================================

/// 已知编程语言标识符列表
const KNOWN_LANGUAGES: &[&str] = &[
    "abap", "actionscript", "ada", "apache", "applescript", "arduino",
    "asm", "asp", "assembly", "awk", "bash", "basic", "batch", "bison",
    "c", "c++", "c#", "csharp", "caml", "clojure", "cmake", "coffeescript",
    "cpp", "crystal", "cql", "css", "cuda", "cypher", "d", "dart",
    "delphi", "diff", "django", "docker", "dot", "elixir", "elm",
    "emacs", "erlang", "excel", "f#", "fsharp", "fasta", "fish",
    "flow", "fortran", "fsharp", "gams", "gcode", "gdscript", "genie",
    "gherkin", "git", "glsl", "glyph", "gnuplot", "go", "golo",
    "gradle", "graphql", "groovy", "haskell", "haxe", "hcl", "hlsl",
    "html", "http", "hy", "idl", "ini", "io", "java", "javascript",
    "jinja", "jq", "json", "julia", "kotlin", "latex", "less", "lilypond",
    "lisp", "livescript", "llvm", "lua", "m4", "makefile", "markdown",
    "mathematica", "matlab", "mcfunction", "meson", "minizinc", "mips",
    "modelica", "mojo", "mql4", "mql5", "msl", "mysql", "nasm", "nginx",
    "nim", "nix", "node", "numpy", "ocaml", "octave", "odin", "opencl",
    "opengl", "pascal", "perl", "php", "pkl", "plsql", "postgresql",
    "powershell", "processing", "prolog", "promql", "protobuf", "pug",
    "puppet", "purebasic", "pyramid", "python", "q", "qml", "r",
    "racket", "ragel", "rasql", "reason", "rebol", "red", "restructuredtext",
    "riscv", "robot", "ruby", "rust", "sas", "sass", "scala", "scheme",
    "scilab", "scss", "shell", "smalltalk", "smarty", "snakemake", "solidity",
    "sparql", "sql", "stan", "stata", "stylus", "svelte", "swift",
    "systemverilog", "tcl", "terraform", "tex", "text", "thrift", "toml",
    "turing", "twig", "typescript", "typoscript", "v", "vala", "vbnet",
    "verilog", "vhdl", "vim", "visualbasic", "vue", "wasm", "wdl",
    "webassembly", "wolfram", "x86asm", "xaml", "xml", "xsl", "yaml",
    "yang", "yara", "zephir", "zig",
];

/// 检查语言标识符是否为已知的编程语言
pub fn is_known_language(lang: &str) -> bool {
    let lang = lang.trim().to_lowercase();
    KNOWN_LANGUAGES.contains(&lang.as_str())
}

/// 从 HTML 文档中提取所有代码块
pub fn extract_code_blocks(document: &Html, _config: &ParserConfig) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();

    // 提取 <pre><code> 块
    let pre_selector = match try_parse_selector("pre") {
        Some(s) => s,
        None => return blocks,
    };

    for pre_element in document.select(&pre_selector) {
        if let Some(code_el) = pre_element.child_elements().next()
            && code_el.value().name() == "code" {
                let block = extract_code_block(&code_el, false);
                blocks.push(block);
            }
    }

    // 提取独立的 <code> 元素（内联代码）
    if let Some(code_sel) = try_parse_selector("code") {
        for element in document.select(&code_sel) {
            // 如果已经在 pre 中处理过则跳过
            let parent = element.parent();
            if let Some(parent_id) = parent
                && let Some(parent_ref) = ElementRef::wrap(parent_id)
                    && parent_ref.value().name() == "pre" {
                        continue;
                    }

            let block = extract_code_block(&element, true);
            blocks.push(block);
        }
    }

    blocks
}

/// 从 code 元素中提取代码块
pub fn extract_code_block(element: &ElementRef, is_inline: bool) -> CodeBlock {
    let code: String = element
        .text()
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();

    // 尝试从 class 或 data-lang 属性获取语言
    let language = element
        .value()
        .attr("class")
        .and_then(|cls| {
            // 常见的格式：language-rust, lang-rust, rust
            for part in cls.split_whitespace() {
                if let Some(lang) = part.strip_prefix("language-")
                    && is_known_language(lang) {
                        return Some(lang.to_string());
                    }
                if let Some(lang) = part.strip_prefix("lang-")
                    && is_known_language(lang) {
                        return Some(lang.to_string());
                    }
                if is_known_language(part) {
                    return Some(part.to_string());
                }
            }
            None
        })
        .or_else(|| {
            element
                .value()
                .attr("data-lang")
                .filter(|l| is_known_language(l))
                .map(ToString::to_string)
        });

    let filename = element
        .value()
        .attr("data-filename")
        .or_else(|| element.value().attr("filename"))
        .map(ToString::to_string);

    let line_count = code.lines().count();

    CodeBlock {
        code,
        language,
        line_count,
        is_inline,
        filename,
    }
}

// ============================================================================
// 引用提取
// ============================================================================

/// 从 HTML 文档中提取所有引用（blockquote）
pub fn extract_quotes(document: &Html, _config: &ParserConfig) -> Vec<Quote> {
    let selector = match try_parse_selector("blockquote") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut quotes = Vec::new();

    for element in document.select(&selector) {
        let text: String = element
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();

        if text.is_empty() || text.len() < 10 {
            continue;
        }

        let cite = element
            .value()
            .attr("cite")
            .map(ToString::to_string);

        // 检查内部是否有 <cite> 标签
        let cite_url = if let Some(cite_sel) = try_parse_selector("cite") {
            element
                .select(&cite_sel)
                .next()
                .and_then(|el| {
                    let cite_text: String = el
                        .text()
                        .collect::<Vec<_>>()
                        .join("")
                        .trim()
                        .to_string();
                    if cite_text.is_empty() { None } else { Some(cite_text) }
                })
        } else {
            None
        };

        let mut quote = Quote::new(text);
        if let Some(c) = cite {
            quote.cite = Some(c);
        }
        if let Some(cu) = cite_url {
            quote.cite_url = Some(cu);
        }

        quotes.push(quote);
    }

    quotes
}

// ============================================================================
// 图片提取
// ============================================================================

/// 从 HTML 文档中提取所有图片
pub fn extract_images(document: &Html, base_url: Option<&Url>) -> Vec<Image> {
    let selector = match try_parse_selector("img") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut images = Vec::new();

    for element in document.select(&selector) {
        if let Some(image) = extract_image(&element, base_url) {
            images.push(image);
        }
    }

    images
}

/// 从 img 元素中提取单张图片
pub fn extract_image(element: &ElementRef, base_url: Option<&Url>) -> Option<Image> {
    let src = element.value().attr("src")?;
    let src = src.trim();

    if src.is_empty() {
        return None;
    }

    // 跳过 data: URI 和纯空白
    if src.starts_with("data:") {
        return None;
    }

    let resolved_src = base_url.and_then(|base| resolve_image_url(base, src));

    let alt = element
        .value()
        .attr("alt")
        .unwrap_or("")
        .to_string();

    let title = element
        .value()
        .attr("title")
        .map(ToString::to_string);

    let width = element
        .value()
        .attr("width")
        .and_then(|w| w.parse::<u32>().ok());

    let height = element
        .value()
        .attr("height")
        .and_then(|h| h.parse::<u32>().ok());

    let srcset = element
        .value()
        .attr("srcset")
        .map(ToString::to_string);

    let sizes = element
        .value()
        .attr("sizes")
        .map(ToString::to_string);

    let loading = element
        .value()
        .attr("loading")
        .map(|v| match v {
            "lazy" => ImageLoading::Lazy,
            _ => ImageLoading::Eager,
        })
        .unwrap_or_default();

    let is_decorative = alt.is_empty();

    let mut image = Image {
        src: src.to_string(),
        url: resolved_src,
        alt,
        title,
        width,
        height,
        srcset,
        sizes,
        loading,
        is_decorative,
    };

    if image.srcset.is_some() && image.url.is_none() && image.src.starts_with('/') {
        // 对以 / 开头的 srcset 图片也尝试解析
        if let Some(base) = base_url {
            image.url = resolve_image_url(base, &image.src);
        }
    }

    Some(image)
}

/// 将图片 URL 解析为绝对 URL
pub fn resolve_image_url(base: &Url, src: &str) -> Option<String> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return Some(src.to_string());
    }

    if src.starts_with("//") {
        // 协议相对 URL
        let scheme = base.scheme();
        return Some(format!("{scheme}:{src}"));
    }

    base.join(src).ok().map(|u| u.to_string())
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_document(html: &str) -> Html {
        Html::parse_document(html)
    }

    fn default_config() -> ParserConfig {
        ParserConfig::default()
    }

    // ── 标题提取 ──

    #[test]
    fn test_extract_headings_basic() {
        let html = r#"<html><body>
            <h1 id="main">主标题</h1>
            <h2 class="section">章节一</h2>
            <h3>子章节</h3>
            <h2>章节二</h2>
        </body></html>"#;
        let doc = create_document(html);
        let headings = extract_headings(&doc);

        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "主标题");
        assert_eq!(headings[0].id.as_deref(), Some("main"));
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "章节一");
        assert!(headings[1].classes.contains(&"section".to_string()));
    }

    #[test]
    fn test_extract_headings_empty() {
        let html = r"<html><body><p>没有标题</p></body></html>";
        let doc = create_document(html);
        let headings = extract_headings(&doc);
        assert!(headings.is_empty());
    }

    #[test]
    fn test_extract_headings_ignores_empty() {
        let html = r"<html><body><h1> </h1><h2>内容</h2></body></html>";
        let doc = create_document(html);
        let headings = extract_headings(&doc);
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].text, "内容");
    }

    #[test]
    fn test_get_main_heading_from_h1() {
        let html = r#"<html><body>
            <h1 id="title">页面标题</h1>
            <h2>次要标题</h2>
        </body></html>"#;
        let doc = create_document(html);
        let main = get_main_heading(&doc);
        assert!(main.is_some());
        assert_eq!(main.unwrap().text, "页面标题");
    }

    #[test]
    fn test_get_main_heading_fallback_to_h2() {
        let html = r"<html><body><h2>次级标题</h2></body></html>";
        let doc = create_document(html);
        let main = get_main_heading(&doc);
        assert!(main.is_some());
        assert_eq!(main.unwrap().level, 2);
    }

    #[test]
    fn test_get_main_heading_none() {
        let html = r"<html><body><p>无标题</p></body></html>";
        let doc = create_document(html);
        let main = get_main_heading(&doc);
        assert!(main.is_none());
    }

    #[test]
    fn test_build_outline_simple() {
        let headings = vec![
            Heading::new(1, "H1").with_id("a"),
            Heading::new(2, "H2-1").with_id("b"),
            Heading::new(2, "H2-2").with_id("c"),
            Heading::new(3, "H3").with_id("d"),
        ];
        let outline = build_outline(&headings);
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].heading.text, "H1");
        assert_eq!(outline[0].children.len(), 2);
        assert_eq!(outline[0].children[0].children.len(), 0);
        assert_eq!(outline[0].children[1].children.len(), 1);
        assert_eq!(outline[0].children[1].children[0].heading.text, "H3");
    }

    #[test]
    fn test_build_outline_multiple_h1() {
        let headings = vec![
            Heading::new(1, "第一部分"),
            Heading::new(2, "细节"),
            Heading::new(1, "第二部分"),
        ];
        let outline = build_outline(&headings);
        assert_eq!(outline.len(), 2);
        assert_eq!(outline[0].heading.text, "第一部分");
        assert_eq!(outline[1].heading.text, "第二部分");
    }

    // ── 段落提取 ──

    #[test]
    fn test_extract_paragraphs_basic() {
        let html = r"<html><body>
            <p>第一段文字内容。</p>
            <p>第二段文字内容，长度足够长。</p>
            <p>短</p>
        </body></html>";
        let doc = create_document(html);
        let paragraphs = extract_paragraphs(&doc, 5);
        assert_eq!(paragraphs.len(), 2);
        assert!(paragraphs[0].contains("第一段"));
    }

    #[test]
    fn test_extract_paragraphs_empty() {
        let html = r"<html><body></body></html>";
        let doc = create_document(html);
        let paragraphs = extract_paragraphs(&doc, 1);
        assert!(paragraphs.is_empty());
    }

    // ── 列表提取 ──

    #[test]
    fn test_extract_lists_unordered() {
        let html = r"<html><body>
            <ul>
                <li>苹果</li>
                <li>香蕉</li>
                <li>樱桃</li>
            </ul>
        </body></html>";
        let doc = create_document(html);
        let lists = extract_lists(&doc, &default_config());
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].list_type, ListType::Unordered);
        assert_eq!(lists[0].items.len(), 3);
        assert_eq!(lists[0].items[0].text, "苹果");
    }

    #[test]
    fn test_extract_lists_ordered() {
        let html = r"<html><body>
            <ol>
                <li>第一</li>
                <li>第二</li>
            </ol>
        </body></html>";
        let doc = create_document(html);
        let lists = extract_lists(&doc, &default_config());
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].list_type, ListType::Ordered);
    }

    #[test]
    fn test_extract_list_with_nested() {
        let html = r"<html><body>
            <ul>
                <li>
                    水果
                    <ul>
                        <li>苹果</li>
                        <li>香蕉</li>
                    </ul>
                </li>
                <li>蔬菜</li>
            </ul>
        </body></html>";
        let doc = create_document(html);
        let lists = extract_lists(&doc, &default_config());
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].items.len(), 2);
        let nested = &lists[0].items[0];
        assert!(nested.nested.is_some());
        let inner = nested.nested.as_ref().unwrap();
        assert_eq!(inner.items.len(), 2);
        assert_eq!(inner.items[0].text, "苹果");
    }

    #[test]
    fn test_extract_list_skips_empty_items() {
        let html = r"<html><body>
            <ul>
                <li></li>
                <li>有效项</li>
            </ul>
        </body></html>";
        let doc = create_document(html);
        let lists = extract_lists(&doc, &default_config());
        assert_eq!(lists[0].items.len(), 1);
    }

    #[test]
    fn test_extract_definition_list_basic() {
        let html = r"<html><body>
            <dl>
                <dt>HTML</dt>
                <dd>超文本标记语言</dd>
                <dt>CSS</dt>
                <dd>层叠样式表</dd>
            </dl>
        </body></html>";
        let doc = create_document(html);
        let lists = extract_lists(&doc, &default_config());
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].list_type, ListType::Definition);
        assert_eq!(lists[0].items.len(), 2);
        assert!(lists[0].items[0].text.contains("HTML"));
        assert!(lists[0].items[0].text.contains("超文本标记语言"));
    }

    #[test]
    fn test_extract_definition_list_dt_only() {
        let html = r"<html><body>
            <dl>
                <dt>术语一</dt>
                <dt>术语二</dt>
            </dl>
        </body></html>";
        let doc = create_document(html);
        let lists = extract_lists(&doc, &default_config());
        assert!(lists.is_empty() || lists[0].items.is_empty());
    }

    // ── 表格提取 ──

    #[test]
    fn test_extract_tables_basic() {
        let html = r"<html><body>
            <table>
                <thead>
                    <tr><th>姓名</th><th>年龄</th></tr>
                </thead>
                <tbody>
                    <tr><td>张三</td><td>25</td></tr>
                    <tr><td>李四</td><td>30</td></tr>
                </tbody>
            </table>
        </body></html>";
        let doc = create_document(html);
        let tables = extract_tables(&doc, &default_config());
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].headers.len(), 1);
        assert_eq!(tables[0].rows.len(), 2);
        assert_eq!(tables[0].column_count, 2);
        assert_eq!(tables[0].rows[0].cells[0].content, "张三");
    }

    #[test]
    fn test_extract_tables_no_thead() {
        let html = r"<html><body>
            <table>
                <tr><td>A</td><td>B</td></tr>
                <tr><td>C</td><td>D</td></tr>
            </table>
        </body></html>";
        let doc = create_document(html);
        let tables = extract_tables(&doc, &default_config());
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn test_extract_table_cell_with_colspan() {
        let html = r#"<html><body><table>
            <tr><td colspan="2">合并单元格</td></tr>
        </table></body></html>"#;
        let doc = create_document(html);
        let tables = extract_tables(&doc, &default_config());
        assert_eq!(tables[0].rows[0].cells[0].colspan, 2);
    }

    #[test]
    fn test_extract_tables_empty() {
        let doc = create_document("<html></html>");
        let tables = extract_tables(&doc, &default_config());
        assert!(tables.is_empty());
    }

    #[test]
    fn test_extract_table_with_caption() {
        let html = r"<html><body><table>
            <caption>用户信息表</caption>
            <tr><th>ID</th><th>名称</th></tr>
        </table></body></html>";
        let doc = create_document(html);
        let tables = extract_tables(&doc, &default_config());
        assert_eq!(tables[0].caption.as_deref(), Some("用户信息表"));
    }

    // ── 代码块提取 ──

    #[test]
    fn test_extract_code_blocks_pre_code() {
        let html = r#"<html><body>
            <pre><code class="language-rust">fn main() { println!("hello"); }</code></pre>
        </body></html>"#;
        let doc = create_document(html);
        let blocks = extract_code_blocks(&doc, &default_config());
        assert_eq!(blocks.len(), 1);
        assert!(!blocks[0].is_inline);
        assert_eq!(blocks[0].language.as_deref(), Some("rust"));
        assert!(blocks[0].code.contains("fn main"));
    }

    #[test]
    fn test_extract_code_blocks_inline() {
        let html = r"<html><body>
            <p>使用 <code>println!</code> 宏输出</p>
        </body></html>";
        let doc = create_document(html);
        let blocks = extract_code_blocks(&doc, &default_config());
        let inline_blocks: Vec<_> = blocks.iter().filter(|b| b.is_inline).collect();
        assert!(!inline_blocks.is_empty());
    }

    #[test]
    fn test_extract_code_blocks_skips_inline_inside_pre() {
        let html = r"<html><body>
            <pre><code>块级代码</code></pre>
            <p><code>内联代码</code></p>
        </body></html>";
        let doc = create_document(html);
        let blocks = extract_code_blocks(&doc, &default_config());
        let inline_blocks: Vec<_> = blocks.iter().filter(|b| b.is_inline).collect();
        let pre_blocks: Vec<_> = blocks.iter().filter(|b| !b.is_inline).collect();
        assert_eq!(pre_blocks.len(), 1);
        assert_eq!(inline_blocks.len(), 1);
    }

    #[test]
    fn test_extract_code_block_with_data_lang() {
        let html = r#"<pre><code data-lang="python">print("hello")</code></pre>"#;
        let doc = create_document(html);
        let blocks = extract_code_blocks(&doc, &default_config());
        assert_eq!(blocks[0].language.as_deref(), Some("python"));
    }

    #[test]
    fn test_extract_code_block_with_filename() {
        let html = r#"<pre><code data-filename="main.rs">fn main() {}</code></pre>"#;
        let doc = create_document(html);
        let blocks = extract_code_blocks(&doc, &default_config());
        assert_eq!(blocks[0].filename.as_deref(), Some("main.rs"));
    }

    #[test]
    fn test_is_known_language_positive() {
        assert!(is_known_language("rust"));
        assert!(is_known_language("python"));
        assert!(is_known_language("javascript"));
        assert!(is_known_language("Rust"));
        assert!(is_known_language("  python  "));
    }

    #[test]
    fn test_is_known_language_negative() {
        assert!(!is_known_language("foobarbaz"));
        assert!(!is_known_language(""));
        assert!(!is_known_language("  "));
    }

    // ── 引用提取 ──

    #[test]
    fn test_extract_quotes_basic() {
        let html = r#"<html><body>
            <blockquote cite="https://example.com">
                这是一段引用文字。
            </blockquote>
        </body></html>"#;
        let doc = create_document(html);
        let quotes = extract_quotes(&doc, &default_config());
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].text.trim(), "这是一段引用文字。");
        assert_eq!(quotes[0].cite.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_extract_quotes_with_inner_cite() {
        let html = r"<html><body>
            <blockquote>
                知识就是力量。
                <cite>弗朗西斯·培根</cite>
            </blockquote>
        </body></html>";
        let doc = create_document(html);
        let quotes = extract_quotes(&doc, &default_config());
        assert!(quotes[0].cite_url.is_some());
    }

    #[test]
    fn test_extract_quotes_filters_short() {
        let html = r"<html><body>
            <blockquote>短</blockquote>
            <blockquote>这是一段足够长的引用文字，长度超过十个字符。</blockquote>
        </body></html>";
        let doc = create_document(html);
        let quotes = extract_quotes(&doc, &default_config());
        assert_eq!(quotes.len(), 1);
    }

    #[test]
    fn test_extract_quotes_empty() {
        let doc = create_document("<html></html>");
        let quotes = extract_quotes(&doc, &default_config());
        assert!(quotes.is_empty());
    }

    // ── 图片提取 ──

    #[test]
    fn test_extract_images_basic() {
        let html = r#"<html><body>
            <img src="/images/photo.jpg" alt="风景照" width="800" height="600">
        </body></html>"#;
        let base = Url::parse("https://example.com").ok();
        let doc = create_document(html);
        let images = extract_images(&doc, base.as_ref());
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].src, "/images/photo.jpg");
        assert_eq!(images[0].alt, "风景照");
        assert_eq!(images[0].width, Some(800));
        assert_eq!(images[0].height, Some(600));
    }

    #[test]
    fn test_extract_images_resolves_url() {
        let html = r#"<html><body><img src="/images/pic.jpg" alt="图片"></body></html>"#;
        let base = Url::parse("https://example.com/blog/").ok();
        let doc = create_document(html);
        let images = extract_images(&doc, base.as_ref());
        assert_eq!(
            images[0].url.as_deref(),
            Some("https://example.com/images/pic.jpg")
        );
    }

    #[test]
    fn test_extract_images_absolute_url() {
        let html = r#"<html><body>
            <img src="https://cdn.example.com/logo.png" alt="Logo">
        </body></html>"#;
        let base = Url::parse("https://example.com").ok();
        let doc = create_document(html);
        let images = extract_images(&doc, base.as_ref());
        assert_eq!(
            images[0].url.as_deref(),
            Some("https://cdn.example.com/logo.png")
        );
    }

    #[test]
    fn test_extract_images_skips_data_uri() {
        let html = r#"<html><body>
            <img src="data:image/png;base64,abc123" alt="内嵌">
        </body></html>"#;
        let doc = create_document(html);
        let images = extract_images(&doc, None);
        assert!(images.is_empty());
    }

    #[test]
    fn test_extract_images_no_src() {
        let html = r#"<html><body><img alt="无src"></body></html>"#;
        let doc = create_document(html);
        let images = extract_images(&doc, None);
        assert!(images.is_empty());
    }

    #[test]
    fn test_extract_image_with_srcset() {
        let html = r#"<html><body>
            <img src="/img/photo.jpg" alt="响应式图片"
                 srcset="/img/photo-400.jpg 400w, /img/photo-800.jpg 800w"
                 sizes="(max-width: 600px) 400px, 800px"
                 loading="lazy">
        </body></html>"#;
        let base = Url::parse("https://example.com").ok();
        let doc = create_document(html);
        let images = extract_images(&doc, base.as_ref());
        assert_eq!(images.len(), 1);
        assert!(images[0].srcset.is_some());
        assert!(images[0].sizes.is_some());
        assert_eq!(images[0].loading, ImageLoading::Lazy);
    }

    #[test]
    fn test_extract_image_decorative() {
        let html = r#"<html><body><img src="/spacer.gif" alt=""></body></html>"#;
        let doc = create_document(html);
        let images = extract_images(&doc, None);
        assert!(images[0].is_decorative);
    }

    #[test]
    fn test_resolve_image_url_absolute() {
        let base = Url::parse("https://example.com").unwrap();
        assert_eq!(
            resolve_image_url(&base, "https://other.com/img.jpg"),
            Some("https://other.com/img.jpg".to_string())
        );
    }

    #[test]
    fn test_resolve_image_url_relative() {
        let base = Url::parse("https://example.com/blog/").unwrap();
        assert_eq!(
            resolve_image_url(&base, "img.jpg"),
            Some("https://example.com/blog/img.jpg".to_string())
        );
    }

    #[test]
    fn test_resolve_image_url_root_relative() {
        let base = Url::parse("https://example.com/blog/").unwrap();
        assert_eq!(
            resolve_image_url(&base, "/images/img.jpg"),
            Some("https://example.com/images/img.jpg".to_string())
        );
    }

    #[test]
    fn test_resolve_image_url_protocol_relative() {
        let base = Url::parse("https://example.com").unwrap();
        assert_eq!(
            resolve_image_url(&base, "//cdn.example.com/img.jpg"),
            Some("https://cdn.example.com/img.jpg".to_string())
        );
    }
}
