//! 文本提取模块
//!
//! 从 HTML 文档中提取纯文本内容，包含清洗、可读性分析和语言检测。
//! 改编自 halldyll-parser 的文本提取实现。

use scraper::{Html, ElementRef, Node};
use std::collections::HashSet;

use crate::selector::{try_parse_selector, BOILERPLATE_SELECTORS, CONTENT_SELECTORS};
use crate::types::{TextContent, ParserConfig, ParserResult};

/// 从 HTML 文档中提取文本内容
pub fn extract_text(document: &Html, config: &ParserConfig) -> ParserResult<TextContent> {
    let raw_text = extract_main_content(document, config);
    let cleaned_text = strip_html_tags(&raw_text);
    let word_count = count_words(&cleaned_text);
    let char_count = cleaned_text.chars().count();
    let language = detect_language(&cleaned_text);
    let readability_score = if config.compute_readability && word_count > 0 {
        Some(flesch_reading_ease(&cleaned_text))
    } else {
        None
    };
    let reading_time_minutes = if word_count > 0 {
        Some(word_count as f64 / 225.0)
    } else {
        None
    };

    Ok(TextContent {
        raw_text,
        cleaned_text,
        word_count,
        char_count,
        language,
        readability_score,
        reading_time_minutes,
    })
}

/// 使用内容选择器提取正文，失败时回退到 body 文本提取
fn extract_main_content(document: &Html, config: &ParserConfig) -> String {
    for selector_str in CONTENT_SELECTORS {
        if let Some(sel) = try_parse_selector(selector_str)
            && let Some(el) = document.select(&sel).next()
        {
            let text = extract_element_text_filtered(el, &BOILERPLATE_SELECTORS);
            if text.len() > config.min_paragraph_length {
                return text;
            }
        }
    }
    for selector_str in &config.content_selectors {
        if let Some(sel) = try_parse_selector(selector_str)
            && let Some(el) = document.select(&sel).next()
        {
            let text = extract_element_text_filtered(el, &BOILERPLATE_SELECTORS);
            if text.len() > config.min_paragraph_length {
                return text;
            }
        }
    }
    if let Some(body_sel) = try_parse_selector("body")
        && let Some(body) = document.select(&body_sel).next()
    {
        extract_body_text(body)
    } else {
        extract_element_text_filtered(document.root_element(), &BOILERPLATE_SELECTORS)
    }
}

/// 从 body 元素提取文本，带基础过滤
fn extract_body_text(element: ElementRef) -> String {
    let mut skip = BOILERPLATE_SELECTORS.clone();
    skip.insert("header");
    skip.insert("footer");
    skip.insert("nav");
    skip.insert("aside");
    extract_element_text_filtered(element, &skip)
}

/// 提取元素的所有文本内容，自动在块级元素后换行
pub fn extract_element_text(element: ElementRef) -> String {
    let mut result = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                let trimmed = normalize_text(text);
                if !trimmed.is_empty() {
                    result.push_str(&trimmed);
                    result.push(' ');
                }
            }
            Node::Element(el) => {
                let tag = el.name.local.as_ref();
                if is_block_element(tag)
                    && !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                if let Some(child_ref) = ElementRef::wrap(child) {
                    result.push_str(&extract_element_text(child_ref));
                }
                if is_block_element(tag)
                    && !result.ends_with('\n') {
                        result.push('\n');
                    }
            }
            _ => {}
        }
    }
    collapse_newlines(&result)
}

/// 提取元素文本并跳过指定的元素类型
fn extract_element_text_filtered(
    element: ElementRef,
    skip_tags: &HashSet<&str>,
) -> String {
    let mut result = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                let trimmed = normalize_text(text);
                if !trimmed.is_empty() {
                    if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('\n') {
                        result.push(' ');
                    }
                    result.push_str(&trimmed);
                }
            }
            Node::Element(el) => {
                let tag = el.name.local.as_ref();
                if skip_tags.contains(tag) {
                    continue;
                }
                if is_block_element(tag)
                    && !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                if let Some(child_ref) = ElementRef::wrap(child) {
                    result.push_str(&extract_element_text_filtered(child_ref, skip_tags));
                }
                if is_block_element(tag) && !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            _ => {}
        }
    }
    collapse_newlines(&result)
}

/// 深度受限的递归文本提取，防止栈溢出
pub fn extract_text_recursive(element: ElementRef, depth: usize, max_depth: usize) -> String {
    if depth >= max_depth {
        return String::new();
    }
    let mut result = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                let trimmed = normalize_text(text);
                if !trimmed.is_empty() {
                    if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('\n') {
                        result.push(' ');
                    }
                    result.push_str(&trimmed);
                }
            }
            Node::Element(el) => {
                let tag = el.name.local.as_ref();
                if should_skip_element_by_tag(tag) {
                    continue;
                }
                if is_block_element(tag) && !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
                if let Some(child_ref) = ElementRef::wrap(child) {
                    result.push_str(&extract_text_recursive(child_ref, depth + 1, max_depth));
                }
                if is_block_element(tag) && !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            _ => {}
        }
    }
    collapse_newlines(&result)
}

// ── 文本处理 ────────────────────────────────────────────────

/// 规范化文本：去除首尾空白，合并内部连续空白为单个空格
pub fn normalize_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                result.push(' ');
                prev_ws = true;
            }
        } else {
            result.push(c);
            prev_ws = false;
        }
    }
    result.trim().to_string()
}

/// 将连续换行折叠为最多两个换行
fn collapse_newlines(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut newline_count = 0;
    for c in text.chars() {
        if c == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                result.push(c);
            }
        } else {
            newline_count = 0;
            result.push(c);
        }
    }
    result.trim().to_string()
}

/// 移除 HTML 标签，保留文本内容
pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    normalize_text(&result)
}

// ── 元素分类 ────────────────────────────────────────────────

/// 判断元素是否应该被跳过
pub fn should_skip_element(element: &ElementRef) -> bool {
    let tag = element.value().name.local.as_ref();
    should_skip_element_by_tag(tag)
}

pub fn should_skip_element_by_tag(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "noscript" | "iframe" | "canvas"
            | "svg" | "math" | "template" | "object" | "embed"
    )
}

/// 判断是否是块级元素
pub fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "ul" | "ol" | "li" | "blockquote" | "pre"
            | "table" | "tr" | "td" | "th" | "section"
            | "article" | "figure" | "figcaption" | "details"
            | "summary" | "dialog" | "hr" | "br" | "address"
            | "fieldset" | "main" | "header" | "footer" | "nav"
            | "aside" | "form" | "dl" | "dt" | "dd"
    )
}

/// 判断是否是行内元素
pub fn is_inline_element(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "span" | "strong" | "em" | "b" | "i" | "u" | "s"
            | "sub" | "sup" | "code" | "kbd" | "q" | "cite"
            | "abbr" | "time" | "mark" | "small" | "del" | "ins"
            | "label" | "button" | "input" | "select" | "textarea"
            | "img" | "br" | "wbr" | "bdi" | "bdo" | "data"
            | "dfn" | "output" | "progress" | "meter" | "ruby"
            | "rp" | "rt" | "samp" | "var" | "acronym"
    )
}

// ── 可读性分析 ───────────────────────────────────────────────

/// 计算 Flesch Reading Ease 分数（0-100，越高越易读）
pub fn flesch_reading_ease(text: &str) -> f64 {
    let word_count = count_words(text) as f64;
    let sentence_count = count_sentences(text) as f64;
    let syllable_count = count_syllables(text) as f64;
    if word_count == 0.0 || sentence_count == 0.0 {
        return 0.0;
    }
    let score = 206.835
        - 1.015 * (word_count / sentence_count)
        - 84.6 * (syllable_count / word_count);
    score.clamp(0.0, 100.0)
}

/// 计算 Flesch-Kincaid Grade Level（美国年级水平）
pub fn flesch_kincaid_grade(text: &str) -> f64 {
    let word_count = count_words(text) as f64;
    let sentence_count = count_sentences(text) as f64;
    let syllable_count = count_syllables(text) as f64;
    if word_count == 0.0 || sentence_count == 0.0 {
        return 0.0;
    }
    let grade = 0.39 * (word_count / sentence_count)
        + 11.8 * (syllable_count / word_count)
        - 15.59;
    grade.max(0.0)
}

/// 统计单词数
pub fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphabetic))
        .count()
}

/// 统计句子数
pub fn count_sentences(text: &str) -> usize {
    let count = text.chars()
        .filter(|&c| c == '.' || c == '!' || c == '?' || c == '。' || c == '！' || c == '？')
        .count();
    count.max(1)
}

/// 统计总音节数
fn count_syllables(text: &str) -> usize {
    text.split_whitespace()
        .map(count_word_syllables)
        .sum()
}

/// 统计单个单词的音节数
fn count_word_syllables(word: &str) -> usize {
    let w = word.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
    if w.is_empty() {
        return 0;
    }
    let len = w.len();
    if len <= 3 {
        return 1;
    }
    let chars: Vec<char> = w.chars().collect();
    let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
    let mut count = 0;
    let mut prev_vowel = false;
    for &c in &chars {
        let is_v = vowels.contains(&c);
        if is_v && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_v;
    }
    if w.ends_with('e') && count > 1 {
        count -= 1;
    }
    if w.ends_with("le") && chars.len() > 2 && !vowels.contains(&chars[chars.len() - 3]) {
        count += 1;
    }
    if (w.ends_with("es") || w.ends_with("ed"))
        && count > 1 {
            count -= 1;
        }
    count.max(1)
}

// ── 语言检测 ─────────────────────────────────────────────────

/// 简单语言检测：基于高频词列表匹配
pub fn detect_language(text: &str) -> Option<String> {
    let words: Vec<String> = text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .filter(|w| w.len() > 2)
        .collect();
    if words.is_empty() {
        return None;
    }
    let en = words.iter().filter(|w| ENGLISH_WORDS.contains(&w.as_str())).count();
    let fr = words.iter().filter(|w| FRENCH_WORDS.contains(&w.as_str())).count();
    let de = words.iter().filter(|w| GERMAN_WORDS.contains(&w.as_str())).count();
    let es = words.iter().filter(|w| SPANISH_WORDS.contains(&w.as_str())).count();
    let scores = [("en", en), ("fr", fr), ("de", de), ("es", es)];
    let best = scores.iter().max_by_key(|(_, count)| *count).unwrap();
    if best.1 == 0 {
        return None;
    }
    Some(best.0.to_string())
}

// ── 语言词表 ─────────────────────────────────────────────────

const ENGLISH_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "can", "had",
    "her", "was", "one", "our", "out", "has", "have", "been", "some", "them",
    "than", "that", "this", "they", "what", "when", "which", "will", "your",
    "about", "into", "over", "such", "with", "would", "could", "should",
    "their", "there", "these", "where", "while", "after", "before", "between",
    "through", "during", "without", "because", "people", "first", "world",
    "still", "every", "great", "think", "thing", "under", "water", "place",
];

const FRENCH_WORDS: &[&str] = &[
    "dans", "pour", "avec", "elle", "ils", "son", "ses", "leur", "nous",
    "vous", "sur", "tout", "plus", "bien", "faire", "être", "avoir",
    "cette", "comme", "mais", "fait", "faites", "entre", "aussi", "temps",
    "monde", "autre", "deux", "grand", "petit", "alors", "tous", "chez",
    "parce", "quand", "donc", "peut", "voir", "sans", "même", "encore",
    "pendant", "toujours", "premier", "jamais", "chaque", "ainsi", "très",
    "quelque", "personne", "homme", "femme", "jour", "nuit", "chose",
];

const GERMAN_WORDS: &[&str] = &[
    "und", "die", "der", "das", "ist", "mit", "auf", "für", "sich", "auch",
    "nicht", "ein", "eine", "einen", "dem", "den", "des", "sie", "sind",
    "aus", "bei", "hat", "haben", "werden", "oder", "nach", "bis", "wir",
    "mir", "wie", "zum", "zur", "durch", "gegen", "schon", "noch", "immer",
    "sein", "seine", "ihre", "ihrer", "dass", "wenn", "aber", "alle", "dann",
    "kann", "soll", "wird", "über", "viel", "groß", "klein", "ganz", "sagen",
    "jetzt", "neue", "erste", "dieser", "dieses", "diese", "anderer", "worden",
];

const SPANISH_WORDS: &[&str] = &[
    "que", "los", "las", "del", "para", "una", "por", "con", "sus", "las",
    "era", "han", "también", "como", "más", "pero", "este", "esta", "entre",
    "todo", "esa", "eso", "cada", "otro", "muy", "todos", "ahora", "desde",
    "hasta", "cuando", "donde", "parte", "después", "durante", "siempre",
    "entonces", "primero", "nunca", "mismo", "porque", "años", "tiempo",
    "forma", "país", "lugar", "mundo", "vida", "día", "casa", "hombre",
    "mujer", "agua", "gran", "ser", "estar", "tener", "hacer", "poder",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn create_document(html: &str) -> Html {
        Html::parse_document(html)
    }

    fn default_config() -> ParserConfig {
        ParserConfig::default()
    }

    #[test]
    fn 提取简单段落文本() {
        let html = "<html><body><p>Hello world. This is a test.</p></body></html>";
        let doc = create_document(html);
        let config = default_config();
        let result = extract_text(&doc, &config).unwrap();
        assert!(result.cleaned_text.contains("Hello world"));
        assert!(result.word_count > 0);
    }

    #[test]
    fn 提取多个段落() {
        let html = "<html><body><p>First paragraph.</p><p>Second paragraph.</p></body></html>";
        let doc = create_document(html);
        let config = default_config();
        let result = extract_text(&doc, &config).unwrap();
        assert!(result.raw_text.contains("First paragraph."));
        assert!(result.raw_text.contains("Second paragraph."));
    }

    #[test]
    fn 跳过脚本和样式() {
        let html = r#"<html><body>
            <p>Visible text.</p>
            <script>alert("hidden");</script>
            <style>.hidden {}</style>
            <p>More visible.</p>
        </body></html>"#;
        let doc = create_document(html);
        let config = default_config();
        let result = extract_text(&doc, &config).unwrap();
        assert!(result.cleaned_text.contains("Visible text."));
        assert!(result.cleaned_text.contains("More visible."));
        assert!(!result.cleaned_text.contains("hidden"));
    }

    #[test]
    fn 处理空文档() {
        let html = "<html></html>";
        let doc = create_document(html);
        let config = default_config();
        let result = extract_text(&doc, &config).unwrap();
        assert_eq!(result.word_count, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn 提取文章内容() {
        let html = r"<html><body>
            <article>
                <h1>Title</h1>
                <p>This is the article content that should be long enough to pass the threshold.</p>
                <p>More content here for testing purposes.</p>
            </article>
        </body></html>";
        let doc = create_document(html);
        let config = default_config();
        let result = extract_text(&doc, &config).unwrap();
        assert!(result.cleaned_text.contains("Title"));
        assert!(result.cleaned_text.contains("article content"));
    }

    #[test]
    fn 使用内容选择器提取() {
        let html = r#"<html><body>
            <div class="content">
                <p>Main content area text.</p>
            </div>
            <div class="sidebar">
                <p>Sidebar text should be excluded.</p>
            </div>
        </body></html>"#;
        let doc = create_document(html);
        let config = ParserConfig {
            content_selectors: vec![".content".to_string()],
            ..default_config()
        };
        let result = extract_text(&doc, &config).unwrap();
        assert!(result.cleaned_text.contains("Main content area"));
        assert!(!result.cleaned_text.contains("Sidebar text"));
    }

    #[test]
    fn 归一化空白字符() {
        assert_eq!(normalize_text("  hello   world  "), "hello world");
        assert_eq!(normalize_text("foo\n\n\nbar"), "foo bar");
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn 折叠换行() {
        let input = "line1\n\n\n\n\nline2";
        let result = collapse_newlines(input);
        assert_eq!(result, "line1\n\nline2");
    }

    #[test]
    fn 剥离HTML标签() {
        let html = "<p>Hello <b>world</b></p>";
        let result = strip_html_tags(html);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn 块级元素分类() {
        assert!(is_block_element("p"));
        assert!(is_block_element("div"));
        assert!(is_block_element("h1"));
        assert!(is_block_element("ul"));
        assert!(!is_block_element("span"));
        assert!(!is_block_element("a"));
    }

    #[test]
    fn 行内元素分类() {
        assert!(is_inline_element("span"));
        assert!(is_inline_element("a"));
        assert!(is_inline_element("strong"));
        assert!(!is_inline_element("div"));
        assert!(!is_inline_element("p"));
    }

    #[test]
    fn 跳过元素检测() {
        assert!(should_skip_element_by_tag("script"));
        assert!(should_skip_element_by_tag("style"));
        assert!(!should_skip_element_by_tag("p"));
        assert!(!should_skip_element_by_tag("div"));
    }

    #[test]
    fn 统计单词数() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("123 !@#"), 0);
        assert_eq!(count_words("hello 123 world"), 2);
    }

    #[test]
    fn 统计句子数() {
        assert_eq!(count_sentences("Hello. World!"), 2);
        assert_eq!(count_sentences("Just one"), 1);
        assert_eq!(count_sentences(""), 1);
    }

    #[test]
    fn 统计单词音节数() {
        assert_eq!(count_word_syllables("the"), 1);
        assert_eq!(count_word_syllables("hello"), 2);
        assert_eq!(count_word_syllables("example"), 3);
        assert_eq!(count_word_syllables("simple"), 2);
    }

    #[test]
    fn 可读性分数范围() {
        let text = "The cat sat on the mat. The dog ran in the park. The bird flew up high.";
        let score = flesch_reading_ease(text);
        assert!((0.0..=100.0).contains(&score));
    }

    #[test]
    fn 年级水平非负() {
        let text = "This is a test. It has multiple sentences. For grade level calculation.";
        let grade = flesch_kincaid_grade(text);
        assert!(grade >= 0.0);
    }

    #[test]
    fn 语言检测英语() {
        let text = "the world and the people are all here for you and me";
        let lang = detect_language(text);
        assert_eq!(lang, Some("en".to_string()));
    }

    #[test]
    fn 语言检测法语() {
        let text = "dans le monde avec elle et lui pour nous vous";
        let lang = detect_language(text);
        assert_eq!(lang, Some("fr".to_string()));
    }

    #[test]
    fn 语言检测德语() {
        let text = "und die der das ist mit auf für sich auch nicht";
        let lang = detect_language(text);
        assert_eq!(lang, Some("de".to_string()));
    }

    #[test]
    fn 语言检测西班牙语() {
        let text = "que los las del para una por con sus como más";
        let lang = detect_language(text);
        assert_eq!(lang, Some("es".to_string()));
    }

    #[test]
    fn 未知语言返回none() {
        let text = "xxx zzz yyy www qqq rrr";
        let lang = detect_language(text);
        assert_eq!(lang, None);
    }

    #[test]
    fn 可读性分析与阅读时间() {
        let html = "<html><body><p>This is a substantial paragraph with enough words to compute readability and reading time estimates for the test case.</p></body></html>";
        let doc = create_document(html);
        let config = ParserConfig {
            compute_readability: true,
            ..default_config()
        };
        let result = extract_text(&doc, &config).unwrap();
        assert!(result.readability_score.is_some());
        assert!(result.reading_time_minutes.is_some());
    }

    #[test]
    fn 深度受限递归提取() {
        let html = "<html><body><div><p><span>deep</span></p></div></body></html>";
        let doc = create_document(html);
        let body_sel = try_parse_selector("body").unwrap();
        let body = doc.select(&body_sel).next().unwrap();
        let text = extract_text_recursive(body, 0, 1);
        assert!(text.is_empty() || text.contains("deep"));
    }

    #[test]
    fn 提取文本过滤导航() {
        let html = r"<html><body>
            <nav><p>Navigation</p></nav>
            <main><p>Main content here.</p></main>
        </body></html>";
        let doc = create_document(html);
        let main_sel = try_parse_selector("main").unwrap();
        let main = doc.select(&main_sel).next().unwrap();
        let skip_set: HashSet<&str> = ["nav"].iter().copied().collect();
        let text = extract_element_text_filtered(main, &skip_set);
        assert!(text.contains("Main content here."));
    }

    #[test]
    fn normalize_保留非英文文本() {
        let text = "  Hello   世界   ";
        assert_eq!(normalize_text(text), "Hello 世界");
    }

    #[test]
    fn 可读性分数与文本长度相关() {
        let easy = "The cat sat. The dog ran. The bird flew.";
        let hard = "The neurological manifestation of paroxysmal hypertension demonstrates substantial phenomenological complexity.";
        assert!(flesch_reading_ease(easy) > flesch_reading_ease(hard));
    }

    #[test]
    fn 总音节数统计() {
        let text = "hello world example simple";
        assert!(count_syllables(text) > 0);
        assert_eq!(count_syllables(""), 0);
    }
}
