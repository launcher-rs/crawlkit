//! # 联系方式提取模块
//!
//! 从 HTML 页面中提取邮箱、电话、地址、社交媒体链接等联系信息。
//! 改编自 halldyll-parser 的联系方式提取逻辑。

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use lazy_static::lazy_static;
use std::collections::HashSet;

use crate::types::ParserResult;

// ============================================================================
// 类型定义
// ============================================================================

/// 统一联系方式容器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContactInfo {
    pub emails: Vec<Email>,
    pub phones: Vec<Phone>,
    pub addresses: Vec<Address>,
    pub social_links: Vec<SocialLink>,
    pub business_name: Option<String>,
}

/// 邮箱地址
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub address: String,
    pub label: Option<String>,
    pub source: EmailSource,
}

/// 邮箱来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailSource {
    /// mailto: 链接
    MailtoLink,
    /// 页面正文文本
    PageText,
    /// meta 标签
    MetaTag,
    /// 结构化数据 (JSON-LD / Microdata)
    StructuredData,
}

impl Default for EmailSource {
    fn default() -> Self {
        Self::PageText
    }
}

/// 电话号码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phone {
    pub number: String,
    pub normalized: String,
    pub label: Option<String>,
    pub phone_type: PhoneType,
}

/// 电话类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhoneType {
    /// 固定电话
    Landline,
    /// 手机
    Mobile,
    /// 传真
    Fax,
    /// 免费热线
    TollFree,
    /// 未知
    Unknown,
}

impl Default for PhoneType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// 物理地址
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub raw: String,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub coordinates: Option<Coordinates>,
}

/// 地理坐标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

/// 社交媒体链接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLink {
    pub url: String,
    pub platform: SocialPlatform,
    pub username: Option<String>,
    pub label: Option<String>,
}

/// 社交平台枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocialPlatform {
    Facebook,
    Twitter,
    LinkedIn,
    Instagram,
    YouTube,
    GitHub,
    TikTok,
    Pinterest,
    Snapchat,
    Reddit,
    Telegram,
    WhatsApp,
    WeChat,
    Weibo,
    Other,
}

// ============================================================================
// 正则表达式（惰性初始化）
// ============================================================================

lazy_static! {
    /// 标准的邮箱地址正则（RFC 5322 简化版）
    static ref EMAIL_RE: Regex = Regex::new(
        r#"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}"#
    ).expect("无效的邮箱正则");

    /// 电话号码正则（支持国际格式、常见分隔符）
    static ref PHONE_RE: Regex = Regex::new(
        r#"(?:\+?\d{1,3}[-.\s]?)?\(?\d{2,4}\)?[-.\s]?\d{3,4}[-.\s]?\d{3,4}(?:\s*(?:分机|ext|x|#)\s*\d+)?\b"#
    ).expect("无效的电话正则");

    /// 美国免费热线正则（800/888/877/866/855/844/833）
    static ref TOLLFREE_RE: Regex = Regex::new(
        r#"(?:\+?1[-.\s]?)?(?:8(?:00|88|77|66|55|44|33))[-.\s]?\d{3}[-.\s]?\d{4}"#
    ).expect("无效的免费热线正则");

    /// 中国手机号正则
    static ref CN_MOBILE_RE: Regex = Regex::new(
        r#"1[3-9]\d{9}"#
    ).expect("无效的中国手机正则");

    /// 中国固定电话正则（含区号）
    static ref CN_LANDLINE_RE: Regex = Regex::new(
        r#"0\d{2,3}[-.\s]?\d{7,8}"#
    ).expect("无效的中国固话正则");

    /// 邮政编码正则（通用 5 位及 5+4 格式）
    static ref POSTAL_CODE_RE: Regex = Regex::new(
        r#"\b\d{5}(?:-\d{4})?\b"#
    ).expect("无效的邮编正则");

    /// 中国邮政编码正则（6 位）
    static ref CN_POSTAL_CODE_RE: Regex = Regex::new(
        r#"\b\d{6}\b"#
    ).expect("无效的中国邮编正则");

    /// 社交媒体 URL 正则
    static ref SOCIAL_URL_RE: Regex = Regex::new(
        r#"(?:https?://)?(?:www\.)?(?:facebook|twitter|x|linkedin|instagram|youtube|github|tiktok|pinterest|snapchat|reddit|t\.me|wa\.me|weixin|weibo)\.(?:com|me|io|tv|cn)/(?:[a-zA-Z0-9_.\-]+/?)+"#
    ).expect("无效的社交链接正则");

    /// 联系页面路径正则
    static ref CONTACT_PAGE_RE: Regex = Regex::new(
        r#"(?i)(?:contact|about|support|联系|关于我们|客服)(?:[-_./]?(?:us|me|info|page|support))?"#
    ).expect("无效的联系页面正则");
}

// ============================================================================
// 邮箱提取
// ============================================================================

/// 从 HTML 文档中提取所有联系信息
pub fn extract_contact_info(html: &str) -> ParserResult<ContactInfo> {
    let document = Html::parse_document(html);
    let emails = extract_emails(&document);
    let phones = extract_phones(&document);
    let addresses = extract_addresses(&document);
    let social_links = extract_social_links(&document);
    let business_name = extract_business_name(&document);

    Ok(ContactInfo {
        emails,
        phones,
        addresses,
        social_links,
        business_name,
    })
}

/// 从 HTML 文档中提取邮箱地址
pub fn extract_emails(document: &Html) -> Vec<Email> {
    let mut emails: Vec<Email> = Vec::new();
    let mut seen = HashSet::new();

    // 从 mailto: 链接提取
    if let Ok(sel) = Selector::parse(r#"a[href^="mailto:"]"#) {
        for element in document.select(&sel) {
            if let Some(href) = element.value().attr("href") {
                let address = href.trim_start_matches("mailto:").trim();
                if is_valid_email(address) && seen.insert(address.to_string()) {
                    let label = element.text().collect::<String>().trim().to_string();
                    emails.push(Email {
                        address: address.to_string(),
                        label: if label.is_empty() || label == address { None } else { Some(label) },
                        source: EmailSource::MailtoLink,
                    });
                }
            }
        }
    }

    // 从 meta 标签提取
    if let Ok(sel) = Selector::parse(r#"meta[name="email"], meta[property="email"]"#) {
        for element in document.select(&sel) {
            if let Some(content) = element.value().attr("content") {
                if is_valid_email(content.trim()) && seen.insert(content.trim().to_string()) {
                    emails.push(Email {
                        address: content.trim().to_string(),
                        label: None,
                        source: EmailSource::MetaTag,
                    });
                }
            }
        }
    }

    // 从页面文本提取
    if let Ok(sel) = Selector::parse("body") {
        for element in document.select(&sel) {
            let text = element.text().collect::<String>();
            for cap in EMAIL_RE.captures_iter(&text) {
                let address = cap[0].to_string();
                if is_valid_email(&address) && seen.insert(address.clone()) {
                    emails.push(Email {
                        address,
                        label: None,
                        source: EmailSource::PageText,
                    });
                }
            }
        }
    }

    emails
}

/// 验证邮箱地址有效性（基础格式检查 + 排除常见无效地址）
pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();

    // 基本格式检查
    if !EMAIL_RE.is_match(email) {
        return false;
    }

    // 排除明显无效的地址
    let invalid_prefixes = [
        "example@", "test@", "admin@", "info@",
        "noreply@", "no-reply@", "donotreply@",
    ];
    for prefix in &invalid_prefixes {
        if email.to_lowercase().starts_with(prefix) {
            return false;
        }
    }

    // 排除占位邮箱
    let placeholder_domains = [
        "example.com", "example.org", "example.net",
        "domain.com", "domain.org", "test.com",
        "localhost", "local",
    ];
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() == 2 {
        let domain = parts[1].to_lowercase();
        for placeholder in &placeholder_domains {
            if domain == *placeholder {
                return false;
            }
        }
        // 排除因文本拼接导致的虚假邮箱（TLD 过长或域名段数异常）
        let domain_parts: Vec<&str> = domain.split('.').collect();
        if domain_parts.len() > 3 || domain_parts.last().map_or(false, |tld| tld.len() > 8) {
            return false;
        }
    }

    true
}

/// 提取邮箱附近的标签文本
pub fn extract_email_label(document: &Html, _address: &str) -> Option<String> {
    // 查找 mailto 链接触近的文本
    if let Ok(sel) = Selector::parse(&format!(r#"a[href="mailto:{}"]"#, _address)) {
        for element in document.select(&sel) {
            let text = element.text().collect::<String>().trim().to_string();
            if !text.is_empty() && text != _address {
                return Some(text);
            }
            // 检查父元素中的前置文本
            let mut parent = element.parent();
            while let Some(p) = parent {
                if let Some(parent_ref) = ElementRef::wrap(p) {
                    let parent_text = parent_ref.text().collect::<String>();
                    let cleaned: String = parent_text
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    if !cleaned.is_empty() && cleaned != _address {
                        return Some(parent_text.trim().to_string());
                    }
                }
                parent = p.parent();
            }
        }
    }
    None
}

// ============================================================================
// 电话提取
// ============================================================================

/// 从 HTML 文档中提取电话号码
pub fn extract_phones(document: &Html) -> Vec<Phone> {
    let mut phones: Vec<Phone> = Vec::new();
    let mut seen = HashSet::new();

    // 从 tel: 链接提取
    if let Ok(sel) = Selector::parse(r#"a[href^="tel:"]"#) {
        for element in document.select(&sel) {
            if let Some(href) = element.value().attr("href") {
                let number = href.trim_start_matches("tel:").trim().to_string();
                let normalized = normalize_phone(&number);
                if !number.is_empty() && seen.insert(normalized.clone()) {
                    let label = element.text().collect::<String>().trim().to_string();
                    let phone_type = detect_phone_type(&number);
                    phones.push(Phone {
                        number: number.clone(),
                        normalized,
                        label: if label.is_empty() || label == number { None } else { Some(label) },
                        phone_type,
                    });
                }
            }
        }
    }

    // 从页面文本提取
    if let Ok(sel) = Selector::parse("body") {
        for element in document.select(&sel) {
            let text = element.text().collect::<String>();

            // 免费热线优先匹配
            for cap in TOLLFREE_RE.captures_iter(&text) {
                let number = cap[0].to_string();
                let normalized = normalize_phone(&number);
                if seen.insert(normalized.clone()) {
                    phones.push(Phone {
                        number,
                        normalized,
                        label: None,
                        phone_type: PhoneType::TollFree,
                    });
                }
            }

            // 普通电话匹配
            for cap in PHONE_RE.captures_iter(&text) {
                let number = cap[0].to_string();
                let normalized = normalize_phone(&number);
                if seen.insert(normalized.clone()) {
                    let phone_type = detect_phone_type(&number);
                    phones.push(Phone {
                        number,
                        normalized,
                        label: None,
                        phone_type,
                    });
                }
            }
        }
    }

    phones
}

/// 提取电话号码附近的标签文本
pub fn extract_phone_label(document: &Html, _number: &str) -> Option<String> {
    if let Ok(sel) = Selector::parse(&format!(r#"a[href="tel:{}"]"#, _number)) {
        for element in document.select(&sel) {
            let text = element.text().collect::<String>().trim().to_string();
            if !text.is_empty() && text != _number {
                return Some(text);
            }
        }
    }
    None
}

/// 检测电话号码类型
pub fn detect_phone_type(number: &str) -> PhoneType {
    let cleaned = number.chars().filter(|c| c.is_ascii_digit()).collect::<String>();

    // 免费热线（含 US 1-800 前缀）
    if cleaned.len() >= 10 && cleaned.len() <= 12 {
        let digits = cleaned.as_str();
        let first_three = if digits.starts_with("1") && digits.len() >= 11 {
            &digits[1..4]
        } else {
            &digits[..3]
        };
        match first_three {
            "800" | "888" | "877" | "866" | "855" | "844" | "833" => return PhoneType::TollFree,
            _ => {}
        }
    }

    // 中国手机
    if cleaned.len() == 11 && cleaned.starts_with('1') {
        let second = cleaned.chars().nth(1).unwrap_or('0');
        if ('3'..='9').contains(&second) {
            return PhoneType::Mobile;
        }
    }

    // 中国固话（区号以 0 开头）
    if cleaned.starts_with('0') && cleaned.len() >= 10 && cleaned.len() <= 12 {
        return PhoneType::Landline;
    }

    PhoneType::Unknown
}

/// 标准化电话号码（仅保留数字）
fn normalize_phone(number: &str) -> String {
    number.chars().filter(|c| c.is_ascii_digit()).collect()
}

// ============================================================================
// 地址提取
// ============================================================================

/// 从 HTML 文档中提取地址信息
pub fn extract_addresses(document: &Html) -> Vec<Address> {
    let mut addresses: Vec<Address> = Vec::new();

    // 尝试结构化地址（schema.org/PostalAddress）
    let structured = extract_structured_address(document);
    addresses.extend(structured);

    // 查找 common 地址标签
    if let Ok(sel) = Selector::parse(
        r#"[itemtype*="PostalAddress"], address, .address, .adr, .location, .contact-address"#
    ) {
        for element in document.select(&sel) {
            let raw = element.text().collect::<String>().trim().to_string();
            if !raw.is_empty() && !addresses.iter().any(|a| a.raw == raw) {
                let components = extract_address_components(&raw);
                addresses.push(Address {
                    raw,
                    ..components
                });
            }
        }
    }

    // 兜底：查找页面中包含地址模式的大段文本
    if addresses.is_empty() {
        if let Ok(sel) = Selector::parse("body") {
            for element in document.select(&sel) {
                let text = element.text().collect::<String>();
                let lower = text.to_lowercase();

                // 包含常见地址关键词的文本段
                let keywords = [
                    "street", "road", "avenue", "boulevard", "lane", "drive",
                    "street", "路", "街", "大道", "巷", "弄",
                    "邮编", "postal code", "zip code", "zip",
                ];
                let has_addr_keyword = keywords.iter().any(|k| lower.contains(k));

                // 包含邮政编码
                let has_postal = POSTAL_CODE_RE.is_match(&text)
                    || CN_POSTAL_CODE_RE.is_match(&text);

                if has_addr_keyword && has_postal {
                    let lines: Vec<&str> = text.lines()
                        .filter(|l| {
                            let lt = l.trim().to_lowercase();
                            keywords.iter().any(|k| lt.contains(k))
                                || POSTAL_CODE_RE.is_match(l)
                                || CN_POSTAL_CODE_RE.is_match(l)
                        })
                        .collect();
                    for line in lines {
                        let raw = line.trim().to_string();
                        if !raw.is_empty() && !addresses.iter().any(|a| a.raw == raw) {
                            let components = extract_address_components(&raw);
                            addresses.push(Address { raw, ..components });
                        }
                    }
                }
            }
        }
    }

    addresses
}

/// 从结构化数据（JSON-LD / Microdata）中提取地址
pub fn extract_structured_address(document: &Html) -> Vec<Address> {
    let mut addresses: Vec<Address> = Vec::new();

    // Microdata 格式: itemscope itemtype="http://schema.org/PostalAddress"
    if let Ok(sel) = Selector::parse(r#"[itemscope][itemtype*="PostalAddress"]"#) {
        for element in document.select(&sel) {
            let raw = element.text().collect::<String>().trim().to_string();
            if raw.is_empty() {
                continue;
            }

            let extract_prop = |prop: &str| -> Option<String> {
                let query = format!(r#"[itemprop="{}"]"#, prop);
                if let Ok(inner_sel) = Selector::parse(&query) {
                    for inner in element.select(&inner_sel) {
                        let val = inner.text().collect::<String>().trim().to_string();
                        if !val.is_empty() {
                            // 检查 meta 标签 content 属性
                            if let Some(content) = inner.value().attr("content") {
                                if !content.is_empty() {
                                    return Some(content.to_string());
                                }
                            }
                            return Some(val);
                        }
                    }
                }
                None
            };

            let street = extract_prop("streetAddress");
            let city = extract_prop("addressLocality");
            let state = extract_prop("addressRegion");
            let postal_code = extract_prop("postalCode");
            let country = extract_prop("addressCountry");

            // 提取地理坐标
            let coords = (|| -> Option<Coordinates> {
                let geo_sel = Selector::parse(r#"[itemscope][itemtype*="GeoCoordinates"]"#).ok()?;
                let geo = element.select(&geo_sel).next()?;

                let lat_sel = Selector::parse(r#"[itemprop="latitude"]"#).ok()?;
                let lat = geo.select(&lat_sel).next().and_then(|e| {
                    if let Some(c) = e.value().attr("content") {
                        c.parse::<f64>().ok()
                    } else {
                        let text = e.text().collect::<String>();
                        text.trim().parse::<f64>().ok()
                    }
                })?;

                let lng_sel = Selector::parse(r#"[itemprop="longitude"]"#).ok()?;
                let lng = geo.select(&lng_sel).next().and_then(|e| {
                    if let Some(c) = e.value().attr("content") {
                        c.parse::<f64>().ok()
                    } else {
                        let text = e.text().collect::<String>();
                        text.trim().parse::<f64>().ok()
                    }
                })?;

                Some(Coordinates { latitude: lat, longitude: lng })
            })();

            addresses.push(Address {
                raw,
                street,
                city,
                state,
                postal_code,
                country,
                coordinates: coords,
            });
        }
    }

    addresses
}

/// 从原始文本中提取地址组成部分
pub fn extract_address_components(raw: &str) -> Address {
    let text = raw.trim();

    // 尝试提取邮政编码
    let postal_code = POSTAL_CODE_RE.find(text)
        .or_else(|| CN_POSTAL_CODE_RE.find(text))
        .map(|m| m.as_str().to_string());

    Address {
        raw: text.to_string(),
        street: if text.len() < 200 { Some(text.to_string()) } else { None },
        city: extract_field_between(text, &["city", "城市", "市", "city/state"], ':'),
        state: extract_field_between(text, &["state", "province", "省", "州"], ':'),
        postal_code,
        country: extract_field_between(text, &["country", "国家", "country/region"], ':'),
        coordinates: None,
    }
}

/// 辅助：从文本中提取指定标签后的字段值
fn extract_field_between(text: &str, labels: &[&str], delimiter: char) -> Option<String> {
    let lower = text.to_lowercase();
    for label in labels {
        let label_lower = label.to_lowercase();
        if let Some(pos) = lower.find(&label_lower) {
            let after_tag = &text[pos + label.len()..].trim();
            if let Some(delim_pos) = after_tag.find(delimiter) {
                let value = after_tag[delim_pos + 1..].trim().to_string();
                if !value.is_empty() {
                    // 取到下一个换行或结束
                    let value = value.lines().next().unwrap_or("").trim().to_string();
                    if !value.is_empty() && value.len() < 100 {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// 社交媒体提取
// ============================================================================

/// 从 HTML 文档中提取社交媒体链接
pub fn extract_social_links(document: &Html) -> Vec<SocialLink> {
    let mut links: Vec<SocialLink> = Vec::new();
    let mut seen = HashSet::new();

    // 从 a 标签提取
    if let Ok(sel) = Selector::parse("a[href]") {
        for element in document.select(&sel) {
            if let Some(href) = element.value().attr("href") {
                let trimmed = href.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("javascript:") {
                    continue;
                }

                let platform = detect_social_platform(trimmed);
                if platform == SocialPlatform::Other {
                    continue;
                }

                if seen.insert(trimmed.to_string()) {
                    let username = extract_social_username(trimmed, platform);
                    let label = element.text().collect::<String>().trim().to_string();
                    links.push(SocialLink {
                        url: trimmed.to_string(),
                        platform,
                        username,
                        label: if label.is_empty() { None } else { Some(label) },
                    });
                }
            }
        }
    }

    // 从 meta/link 标签提取
    for attr in &["me", "url", "social"] {
        if let Ok(sel) = Selector::parse(&format!(r#"link[rel="{}"], a[rel="{}"]"#, attr, attr)) {
            for element in document.select(&sel) {
                if let Some(href) = element.value().attr("href") {
                    let platform = detect_social_platform(href);
                    if platform != SocialPlatform::Other && seen.insert(href.to_string()) {
                        let username = extract_social_username(href, platform);
                        links.push(SocialLink {
                            url: href.to_string(),
                            platform,
                            username,
                            label: None,
                        });
                    }
                }
            }
        }
    }

    links
}

/// 检测社交媒体平台类型
pub fn detect_social_platform(url: &str) -> SocialPlatform {
    let lower = url.to_lowercase();

    if lower.contains("facebook") || lower.contains("fb.com") {
        SocialPlatform::Facebook
    } else if lower.contains("twitter") || lower.contains("x.com") {
        SocialPlatform::Twitter
    } else if lower.contains("linkedin") {
        SocialPlatform::LinkedIn
    } else if lower.contains("instagram") {
        SocialPlatform::Instagram
    } else if lower.contains("youtube") || lower.contains("youtu.be") {
        SocialPlatform::YouTube
    } else if lower.contains("github") {
        SocialPlatform::GitHub
    } else if lower.contains("tiktok") {
        SocialPlatform::TikTok
    } else if lower.contains("pinterest") {
        SocialPlatform::Pinterest
    } else if lower.contains("snapchat") {
        SocialPlatform::Snapchat
    } else if lower.contains("reddit") {
        SocialPlatform::Reddit
    } else if lower.contains("t.me") || lower.contains("telegram") {
        SocialPlatform::Telegram
    } else if lower.contains("wa.me") || lower.contains("whatsapp") {
        SocialPlatform::WhatsApp
    } else if lower.contains("weixin") || lower.contains("wechat") {
        SocialPlatform::WeChat
    } else if lower.contains("weibo") {
        SocialPlatform::Weibo
    } else {
        SocialPlatform::Other
    }
}

/// 从社交 URL 中提取用户名
pub fn extract_social_username(url: &str, platform: SocialPlatform) -> Option<String> {
    let url = url.trim_end_matches('/');

    match platform {
        SocialPlatform::Twitter | SocialPlatform::Facebook | SocialPlatform::Instagram
        | SocialPlatform::TikTok | SocialPlatform::Pinterest | SocialPlatform::Snapchat => {
            // 取路径最后一段
            let parts: Vec<&str> = url.split('/').collect();
            let last = parts.iter().rev().find(|s| !s.is_empty() && **s != "www" && !s.contains('.'))?;
            let username = last.trim_start_matches('@').to_string();
            if !username.is_empty() && !username.contains('#') && !username.contains('?') {
                Some(username)
            } else {
                None
            }
        }
        SocialPlatform::LinkedIn => {
            // linkedin.com/in/username 或 linkedin.com/company/name
            let parts: Vec<&str> = url.split('/').collect();
            if let Some(idx) = parts.iter().position(|s| *s == "in" || *s == "company") {
                parts.get(idx + 1).map(|s| s.to_string())
            } else {
                None
            }
        }
        SocialPlatform::GitHub => {
            // github.com/username
            let parts: Vec<&str> = url.split('/').collect();
            parts.get(3).map(|s| s.to_string()).filter(|s| !s.is_empty())
        }
        SocialPlatform::Telegram => {
            // t.me/username 或 t.me/s/username
            let parts: Vec<&str> = url.split('/').collect();
            let last = parts.last()?;
            if *last != "s" && !last.is_empty() {
                Some(last.to_string())
            } else {
                None
            }
        }
        SocialPlatform::YouTube | SocialPlatform::Reddit
        | SocialPlatform::WhatsApp | SocialPlatform::WeChat
        | SocialPlatform::Weibo | SocialPlatform::Other => None,
    }
}

// ============================================================================
// 实用工具函数
// ============================================================================

/// 查找页面中的联系方式页面链接
pub fn find_contact_page(document: &Html) -> Option<String> {
    // 从 a 标签查找
    if let Ok(sel) = Selector::parse("a[href]") {
        for element in document.select(&sel) {
            if let Some(href) = element.value().attr("href") {
                let href = href.trim();
                let text = element.text().collect::<String>();
                if CONTACT_PAGE_RE.is_match(href) || CONTACT_PAGE_RE.is_match(&text) {
                    return Some(href.to_string());
                }
            }
        }
    }

    // 从 nav / header 查找
    if let Ok(sel) = Selector::parse("nav a[href], header a[href], .nav a[href]") {
        for element in document.select(&sel) {
            if let Some(href) = element.value().attr("href") {
                let href = href.trim();
                let text = element.text().collect::<String>();
                if CONTACT_PAGE_RE.is_match(href) || CONTACT_PAGE_RE.is_match(&text) {
                    return Some(href.to_string());
                }
            }
        }
    }

    None
}

/// 从 HTML 文档中提取企业/组织名称
pub fn extract_business_name(document: &Html) -> Option<String> {
    // 从 schema.org/Organization 提取
    if let Ok(sel) = Selector::parse(r#"[itemscope][itemtype*="Organization"] [itemprop="name"]"#) {
        for element in document.select(&sel) {
            if let Some(content) = element.value().attr("content") {
                let name = content.trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            let text = element.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    // 从 OpenGraph site_name 提取
    if let Ok(sel) = Selector::parse(r#"meta[property="og:site_name"]"#) {
        for element in document.select(&sel) {
            if let Some(content) = element.value().attr("content") {
                let name = content.trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }

    // 从页面标题提取（取第一个 | - 分隔符前的部分）
    if let Ok(sel) = Selector::parse("title") {
        for element in document.select(&sel) {
            let title = element.text().collect::<String>();
            let title = title.trim();
            if !title.is_empty() {
                // 尝试分割标题
                for sep in &[" | ", " - ", " – ", " — ", " |", " -"] {
                    if let Some(name) = title.split(sep).next() {
                        let name = name.trim();
                        if !name.is_empty() && name.len() < 80 {
                            return Some(name.to_string());
                        }
                    }
                }
                return Some(title.to_string());
            }
        }
    }

    None
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 检查 HTML 文档中是否包含任何联系信息
pub fn has_contact_info(html: &str) -> bool {
    let document = Html::parse_document(html);
    !extract_emails(&document).is_empty()
        || !extract_phones(&document).is_empty()
        || !extract_social_links(&document).is_empty()
}

/// 直接获取所有邮箱地址字符串
pub fn get_emails(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    extract_emails(&document)
        .into_iter()
        .map(|e| e.address)
        .collect()
}

/// 直接获取所有电话号码字符串
pub fn get_phones(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    extract_phones(&document)
        .into_iter()
        .map(|p| p.number)
        .collect()
}

/// 直接获取所有社交链接 URL 字符串
pub fn get_social_links(html: &str) -> Vec<(String, SocialPlatform)> {
    let document = Html::parse_document(html);
    extract_social_links(&document)
        .into_iter()
        .map(|s| (s.url, s.platform))
        .collect()
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_info_empty() {
        let info = extract_contact_info("<body></body>").unwrap();
        assert!(info.emails.is_empty() && info.phones.is_empty() && info.addresses.is_empty());
    }

    #[test]
    fn test_contact_info_full() {
        let html = r#"<html><head><meta property="og:site_name" content="测试公司"><meta name="email" content="contact@realcompany.com"></head><body><a href="mailto:info@realcompany.com">联系我们</a><a href="tel:+8613800138000">客服热线</a><a href="https://twitter.com/testcorp">Twitter</a></body></html>"#;
        let info = extract_contact_info(html).unwrap();
        assert!(!info.emails.is_empty() && !info.phones.is_empty() && !info.social_links.is_empty());
        assert_eq!(info.business_name.as_deref(), Some("测试公司"));
    }

    #[test]
    fn test_extract_emails_from_mailto() {
        let doc = Html::parse_document(r#"<a href="mailto:hello@realcompany.com">Send email</a>"#);
        let emails = extract_emails(&doc);
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].address, "hello@realcompany.com");
        assert_eq!(emails[0].source, EmailSource::MailtoLink);
        assert_eq!(emails[0].label.as_deref(), Some("Send email"));
    }

    #[test]
    fn test_extract_emails_from_meta_and_text() {
        let doc = Html::parse_document(r#"<meta name="email" content="contact@realcompany.com"><body>support@realcompany.com</body>"#);
        let emails = extract_emails(&doc);
        assert_eq!(emails.len(), 2);
        assert!(emails.iter().any(|e| e.source == EmailSource::MetaTag));
        assert!(emails.iter().any(|e| e.source == EmailSource::PageText));
    }

    #[test]
    fn test_extract_emails_dedup_and_filter() {
        let doc = Html::parse_document(r#"<html><head><meta name="email" content="dup@realcompany.com"></head><body><a href="mailto:dup@realcompany.com">Email</a><p>dup@realcompany.com</p><!-- placeholder should be filtered --><p>example@example.com</p></body></html>"#);
        let emails = extract_emails(&doc);
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].address, "dup@realcompany.com");
    }

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("user@realcompany.com") && is_valid_email("user.name+tag@domain.co.uk"));
        assert!(!is_valid_email("example@example.com") && !is_valid_email("notanemail") && !is_valid_email("user@localhost"));
    }

    #[test]
    fn test_extract_email_label() {
        let doc = Html::parse_document(r#"<a href="mailto:contact@co.com">业务合作</a>"#);
        assert_eq!(extract_email_label(&doc, "contact@co.com").as_deref(), Some("业务合作"));
    }

    #[test]
    fn test_extract_phones_from_tel_and_text() {
        let doc = Html::parse_document(r#"<a href="tel:+86-10-8888-6666">电话</a><body>Call 1-800-555-0199</body>"#);
        let phones = extract_phones(&doc);
        assert!(!phones.is_empty());
        assert!(phones[0].number.contains("+86"));
        assert_eq!(phones[0].label.as_deref(), Some("电话"));
    }

    #[test]
    fn test_detect_phone_types() {
        assert_eq!(detect_phone_type("1-800-555-0199"), PhoneType::TollFree);
        assert_eq!(detect_phone_type("13800138000"), PhoneType::Mobile);
        assert_eq!(detect_phone_type("010-8888-6666"), PhoneType::Landline);
        assert_eq!(detect_phone_type("12345"), PhoneType::Unknown);
    }

    #[test]
    fn test_extract_phone_label() {
        let doc = Html::parse_document(r#"<a href="tel:+8613800138000">客服</a>"#);
        assert_eq!(extract_phone_label(&doc, "+8613800138000").as_deref(), Some("客服"));
    }

    #[test]
    fn test_extract_structured_address_microdata() {
        let html = r#"<div itemscope itemtype="http://schema.org/PostalAddress"><span itemprop="streetAddress">123 Main St</span><span itemprop="addressLocality">Springfield</span><span itemprop="addressRegion">IL</span><span itemprop="postalCode">62701</span><span itemprop="addressCountry">US</span></div>"#;
        let doc = Html::parse_document(html);
        let addrs = extract_structured_address(&doc);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0].street.as_deref(), Some("123 Main St"));
        assert_eq!(addrs[0].city.as_deref(), Some("Springfield"));
        assert_eq!(addrs[0].state.as_deref(), Some("IL"));
        assert_eq!(addrs[0].postal_code.as_deref(), Some("62701"));
    }

    #[test]
    fn test_extract_address_from_element() {
        let doc = Html::parse_document(r#"<address>北京朝阳区建国路88号 邮编：100025</address>"#);
        let addrs = extract_addresses(&doc);
        assert!(!addrs.is_empty() && addrs[0].raw.contains("北京"));
    }

    #[test]
    fn test_extract_address_components() {
        let a = extract_address_components("123 Main St, Springfield, IL 62701 US");
        assert_eq!(a.postal_code.as_deref(), Some("62701"));
        let b = extract_address_components("北京市海淀区中关村大街1号 邮编：100086");
        assert_eq!(b.postal_code.as_deref(), Some("100086"));
    }

    #[test]
    fn test_extract_structured_address_with_coordinates() {
        let html = r#"<div itemscope itemtype="http://schema.org/PostalAddress"><span itemprop="streetAddress">1600 Amphitheatre Pkwy</span><span itemprop="addressLocality">Mountain View</span><span itemprop="addressRegion">CA</span><span itemprop="postalCode">94043</span><div itemscope itemtype="http://schema.org/GeoCoordinates"><meta itemprop="latitude" content="37.422"><meta itemprop="longitude" content="-122.084"></div></div>"#;
        let doc = Html::parse_document(html);
        let addrs = extract_structured_address(&doc);
        let coords = addrs[0].coordinates.as_ref().unwrap();
        assert!((coords.latitude - 37.422).abs() < 0.001);
        assert!((coords.longitude + 122.084).abs() < 0.001);
    }

    #[test]
    fn test_extract_social_links() {
        let doc = Html::parse_document(r#"<a href="https://twitter.com/testuser">Twitter</a><a href="https://github.com/testuser">GitHub</a><a href="https://linkedin.com/in/testuser">LinkedIn</a>"#);
        let links = extract_social_links(&doc);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].platform, SocialPlatform::Twitter);
    }

    #[test]
    fn test_extract_social_links_dedup() {
        let doc = Html::parse_document(r#"<a href="https://twitter.com/user">A</a><a href="https://twitter.com/user">B</a>"#);
        assert_eq!(extract_social_links(&doc).len(), 1);
    }

    #[test]
    fn test_detect_social_platforms() {
        assert_eq!(detect_social_platform("https://facebook.com/u"), SocialPlatform::Facebook);
        assert_eq!(detect_social_platform("https://x.com/u"), SocialPlatform::Twitter);
        assert_eq!(detect_social_platform("https://linkedin.com/in/u"), SocialPlatform::LinkedIn);
        assert_eq!(detect_social_platform("https://t.me/u"), SocialPlatform::Telegram);
        assert_eq!(detect_social_platform("https://example.com"), SocialPlatform::Other);
    }

    #[test]
    fn test_extract_social_username() {
        assert_eq!(extract_social_username("https://twitter.com/elonmusk", SocialPlatform::Twitter), Some("elonmusk".to_string()));
        assert_eq!(extract_social_username("https://github.com/rust-lang", SocialPlatform::GitHub), Some("rust-lang".to_string()));
        assert_eq!(extract_social_username("https://linkedin.com/in/johndoe", SocialPlatform::LinkedIn), Some("johndoe".to_string()));
        assert_eq!(extract_social_username("https://t.me/rustlang", SocialPlatform::Telegram), Some("rustlang".to_string()));
        assert_eq!(extract_social_username("https://instagram.com/natgeo", SocialPlatform::Instagram), Some("natgeo".to_string()));
        assert_eq!(extract_social_username("https://twitter.com/@user", SocialPlatform::Twitter), Some("user".to_string()));
        assert!(extract_social_username("https://youtube.com/user", SocialPlatform::YouTube).is_none());
    }

    #[test]
    fn test_find_contact_page() {
        let doc = Html::parse_document(r#"<a href="/contact">Contact Us</a>"#);
        assert_eq!(find_contact_page(&doc).as_deref(), Some("/contact"));
        let doc2 = Html::parse_document(r#"<a href="/about/contact">联系我们</a>"#);
        assert_eq!(find_contact_page(&doc2).as_deref(), Some("/about/contact"));
        let doc3 = Html::parse_document(r#"<a href="/products">Products</a>"#);
        assert!(find_contact_page(&doc3).is_none());
    }

    #[test]
    fn test_extract_business_name() {
        let doc1 = Html::parse_document(r#"<meta property="og:site_name" content="ACME 公司">"#);
        assert_eq!(extract_business_name(&doc1).as_deref(), Some("ACME 公司"));
        let doc2 = Html::parse_document("<title>ACME Corp | 创新科技</title>");
        assert_eq!(extract_business_name(&doc2).as_deref(), Some("ACME Corp"));
        let doc3 = Html::parse_document(r#"<div itemscope itemtype="http://schema.org/Organization"><span itemprop="name">大数据有限公司</span></div>"#);
        assert_eq!(extract_business_name(&doc3).as_deref(), Some("大数据有限公司"));
        let doc4 = Html::parse_document("<html><head></head><body></body></html>");
        assert!(extract_business_name(&doc4).is_none());
    }

    #[test]
    fn test_convenience_functions() {
        assert!(has_contact_info(r#"<a href="mailto:a@b.com">Email</a>"#));
        assert!(!has_contact_info("<body>nothing</body>"));

        let html = r#"<a href="mailto:a@b.com">A</a><a href="mailto:c@d.com">C</a>"#;
        assert_eq!(get_emails(html).len(), 2);

        let html2 = r#"<a href="tel:+8613800138000">M</a><a href="tel:+8613900139000">M2</a>"#;
        assert_eq!(get_phones(html2).len(), 2);

        let html3 = r#"<a href="https://twitter.com/u1">T</a><a href="https://github.com/u2">G</a>"#;
        assert_eq!(get_social_links(html3).len(), 2);
    }

    #[test]
    fn test_extract_phones_deduplication() {
        let doc = Html::parse_document(r#"<a href="tel:+8613800138000">Phone</a><body>13800138000</body>"#);
        let phones = extract_phones(&doc);
        // tel:+8613800138000 与 13800138000 规范化后不同（含/不含量国家码）
        assert_eq!(phones.len(), 2);
        assert!(phones.iter().any(|p| p.number.contains("+86")));
        assert!(phones.iter().any(|p| !p.number.contains("+86")));
    }

    #[test]
    fn test_empty_html_edge_cases() {
        assert!(extract_emails(&Html::parse_document("")).is_empty());
        assert!(extract_phones(&Html::parse_document("")).is_empty());
        assert!(extract_social_links(&Html::parse_document("<body></body>")).is_empty());
        assert!(extract_addresses(&Html::parse_document("<body></body>")).is_empty());
        assert!(ContactInfo::default().emails.is_empty());
    }

    #[test]
    fn test_normalize_phone_removes_non_digits() {
        assert_eq!(normalize_phone("+1 (800) 555-0199"), "18005550199");
    }
}
