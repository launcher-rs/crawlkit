//! 文档提取
//!
//! 支持：
//! - PDF 链接
//! - Office 文档（Word、Excel、PowerPoint）
//! - 电子书（EPUB）
//! - 下载链接（<a download>）
//! - 嵌入对象中的 PDF

use scraper::{Html, Selector, ElementRef};
use std::collections::HashSet;
use url::Url;

use crate::types::{
    DocumentMedia, DocumentType, MediaResult,
};

/// 文档扩展名映射表
const DOCUMENT_EXTENSIONS: &[(&str, DocumentType)] = &[
    ("pdf", DocumentType::Pdf),
    ("doc", DocumentType::Word),
    ("docx", DocumentType::Word),
    ("odt", DocumentType::Word),
    ("rtf", DocumentType::Word),
    ("xls", DocumentType::Excel),
    ("xlsx", DocumentType::Excel),
    ("ods", DocumentType::Excel),
    ("csv", DocumentType::Csv),
    ("ppt", DocumentType::PowerPoint),
    ("pptx", DocumentType::PowerPoint),
    ("odp", DocumentType::PowerPoint),
    ("txt", DocumentType::Text),
    ("epub", DocumentType::Epub),
];

/// 从 HTML 文档提取所有文档
pub fn extract_documents(document: &Html, base_url: Option<&Url>) -> Vec<DocumentMedia> {
    let mut documents = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href")
                && is_document_url(href)
                    && let Some(doc) = extract_document_link(&el, base_url) {
                        let key = doc.absolute_url.as_ref().unwrap_or(&doc.url).clone();
                        if seen_urls.insert(key) {
                            documents.push(doc);
                        }
                    }
        }
    }

    if let Ok(sel) = Selector::parse("a[download]") {
        for el in document.select(&sel) {
            if el.value().attr("href").is_some()
                && let Some(doc) = extract_document_link(&el, base_url) {
                    let key = doc.absolute_url.as_ref().unwrap_or(&doc.url).clone();
                    if seen_urls.insert(key) {
                        documents.push(doc);
                    }
                }
        }
    }

    if let Ok(sel) = Selector::parse("object[data*='.pdf'], embed[src*='.pdf']") {
        for el in document.select(&sel) {
            let src = el.value().attr("data").or_else(|| el.value().attr("src"));
            if let Some(src) = src
                && seen_urls.insert(src.to_string()) {
                    let doc = DocumentMedia {
                        url: src.to_string(),
                        absolute_url: resolve_url(src, base_url),
                        doc_type: DocumentType::Pdf,
                        mime_type: Some("application/pdf".to_string()),
                        ..Default::default()
                    };
                    documents.push(doc);
                }
        }
    }

    if let Ok(sel) = Selector::parse("iframe[src*='.pdf']") {
        for el in document.select(&sel) {
            if let Some(src) = el.value().attr("src")
                && seen_urls.insert(src.to_string()) {
                    let doc = DocumentMedia {
                        url: src.to_string(),
                        absolute_url: resolve_url(src, base_url),
                        doc_type: DocumentType::Pdf,
                        title: el.value().attr("title").map(std::string::ToString::to_string),
                        mime_type: Some("application/pdf".to_string()),
                        ..Default::default()
                    };
                    documents.push(doc);
                }
        }
    }

    documents
}

/// 从链接提取文档
fn extract_document_link(el: &ElementRef, base_url: Option<&Url>) -> Option<DocumentMedia> {
    let href = el.value().attr("href")?;
    let absolute_url = resolve_url(href, base_url);
    let doc_type = detect_document_type(href);
    let filename = extract_filename(href);

    let title = el.value().attr("title")
        .map(std::string::ToString::to_string)
        .or_else(|| {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() { Some(text) } else { None }
        })
        .or_else(|| filename.clone());

    let mime_type = guess_document_mime(&doc_type);

    Some(DocumentMedia {
        url: href.to_string(),
        absolute_url,
        doc_type,
        filename,
        title,
        mime_type,
        size_bytes: None,
        page_count: None,
    })
}

/// 判断 URL 是否指向文档
fn is_document_url(url: &str) -> bool {
    let u = url.to_lowercase();
    DOCUMENT_EXTENSIONS.iter().any(|(ext, _)| {
        u.ends_with(&format!(".{ext}")) ||
        u.contains(&format!(".{ext}?")) ||
        u.contains(&format!(".{ext}&"))
    })
}

/// 检测文档类型
fn detect_document_type(url: &str) -> DocumentType {
    let u = url.to_lowercase();

    for (ext, doc_type) in DOCUMENT_EXTENSIONS {
        if u.contains(&format!(".{ext}")) {
            return *doc_type;
        }
    }

    DocumentType::Other
}

/// 从 URL 提取文件名
fn extract_filename(url: &str) -> Option<String> {
    let path = url.split('?').next()?;
    let filename = path.rsplit('/').next()?;

    if filename.is_empty() || !filename.contains('.') {
        return None;
    }

    Some(filename.to_string())
}

/// 猜测文档 MIME 类型
fn guess_document_mime(doc_type: &DocumentType) -> Option<String> {
    match doc_type {
        DocumentType::Pdf => Some("application/pdf".to_string()),
        DocumentType::Word => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string()),
        DocumentType::Excel => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
        DocumentType::PowerPoint => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string()),
        DocumentType::Text => Some("text/plain".to_string()),
        DocumentType::Csv => Some("text/csv".to_string()),
        DocumentType::Epub => Some("application/epub+zip".to_string()),
        DocumentType::Other => None,
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

/// 从 HTML 字符串提取文档
pub fn extract_documents_from_html(html: &str, base_url: Option<&str>) -> MediaResult<Vec<DocumentMedia>> {
    let document = Html::parse_document(html);
    let base = base_url.and_then(|u| Url::parse(u).ok());
    Ok(extract_documents(&document, base.as_ref()))
}

/// 获取所有文档 URL
pub fn get_document_urls(html: &str, base_url: Option<&str>) -> Vec<String> {
    extract_documents_from_html(html, base_url)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|d| d.absolute_url)
        .collect()
}

/// 检查 HTML 是否包含文档
pub fn has_documents(document: &Html) -> bool {
    if let Ok(sel) = Selector::parse("a[href]") {
        document.select(&sel)
            .any(|el| {
                el.value().attr("href")
                    .is_some_and(is_document_url)
            })
    } else {
        false
    }
}

/// 获取 PDF 列表
pub fn get_pdfs(documents: &[DocumentMedia]) -> Vec<&DocumentMedia> {
    documents.iter()
        .filter(|d| d.doc_type == DocumentType::Pdf)
        .collect()
}

/// 获取 Office 文档列表
pub fn get_office_docs(documents: &[DocumentMedia]) -> Vec<&DocumentMedia> {
    documents.iter()
        .filter(|d| matches!(d.doc_type,
            DocumentType::Word | DocumentType::Excel | DocumentType::PowerPoint
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
    fn test_extract_pdf_link() {
        let html = r#"<a href="/documents/report.pdf">Download Report</a>"#;
        let doc = parse_html(html);
        let base = Url::parse("https://example.com").unwrap();
        let documents = extract_documents(&doc, Some(&base));

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].doc_type, DocumentType::Pdf);
        assert_eq!(documents[0].title, Some("Download Report".to_string()));
    }

    #[test]
    fn test_has_documents() {
        let with_docs = r#"<a href="file.pdf">PDF</a>"#;
        let without_docs = r#"<a href="/page">Link</a>"#;

        assert!(has_documents(&parse_html(with_docs)));
        assert!(!has_documents(&parse_html(without_docs)));
    }

    #[test]
    fn test_detect_document_type() {
        assert_eq!(detect_document_type("/file.pdf"), DocumentType::Pdf);
        assert_eq!(detect_document_type("/file.docx"), DocumentType::Word);
        assert_eq!(detect_document_type("/file.xlsx"), DocumentType::Excel);
    }
}
