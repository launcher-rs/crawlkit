//! 通用数据类型定义

use serde::{Deserialize, Serialize};

/// 网站配置
///
/// 用于基于配置的爬取场景，定义单个网站的爬取规则。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SiteConfig {
    /// 网站名称（唯一标识）
    pub name: String,
    /// 网站 URL
    pub url: String,
    /// 文章列表 CSS 选择器
    pub list_selector: String,
    /// 文章链接选择器
    pub link_selector: String,
    /// 文章标题选择器
    pub title_selector: String,
    /// 文章内容选择器
    pub content_selector: String,
    /// 是否启用
    pub enabled: bool,
}

/// 抓取到的文章信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScrapedArticle {
    /// 来源网站名称
    pub site_name: String,
    /// 文章标题
    pub title: String,
    /// 文章 URL
    pub url: String,
    /// 文章原始 HTML
    pub raw_html: Option<String>,
    /// 文章纯文本内容
    pub content: Option<String>,
}

impl ScrapedArticle {
    /// 创建新的文章信息
    pub fn new(site_name: &str, title: &str, url: &str) -> Self {
        Self {
            site_name: site_name.to_string(),
            title: title.to_string(),
            url: url.to_string(),
            raw_html: None,
            content: None,
        }
    }

    /// 设置原始 HTML
    pub fn with_raw_html(mut self, html: &str) -> Self {
        self.raw_html = Some(html.to_string());
        self
    }

    /// 设置文章内容
    pub fn with_content(mut self, content: &str) -> Self {
        self.content = Some(content.to_string());
        self
    }
}

/// 爬取结果统计
#[derive(Debug, Clone, Default)]
pub struct ScrapeStats {
    /// 总共尝试的 URL 数
    pub total: usize,
    /// 成功数
    pub success: usize,
    /// 失败数
    pub failed: usize,
    /// 跳过数（已访问或禁用）
    pub skipped: usize,
}

impl ScrapeStats {
    /// 创建新统计
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录成功
    pub fn record_success(&mut self) {
        self.total += 1;
        self.success += 1;
    }

    /// 记录失败
    pub fn record_failure(&mut self) {
        self.total += 1;
        self.failed += 1;
    }

    /// 记录跳过
    pub fn record_skip(&mut self) {
        self.total += 1;
        self.skipped += 1;
    }

    /// 成功率
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.success as f64 / self.total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_config_serialize_deserialize() {
        let config = SiteConfig {
            name: "test_site".to_string(),
            url: "https://example.com".to_string(),
            list_selector: "article.post".to_string(),
            link_selector: "a.title".to_string(),
            title_selector: "h2".to_string(),
            content_selector: "div.body".to_string(),
            enabled: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SiteConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_scraped_article_serialize_deserialize() {
        let article = ScrapedArticle {
            site_name: "test_site".to_string(),
            title: "测试标题".to_string(),
            url: "https://example.com/article/1".to_string(),
            raw_html: Some("<p>html</p>".to_string()),
            content: Some("正文内容".to_string()),
        };

        let json = serde_json::to_string(&article).unwrap();
        let deserialized: ScrapedArticle = serde_json::from_str(&json).unwrap();
        assert_eq!(article, deserialized);
    }

    #[test]
    fn test_scrape_stats() {
        let mut stats = ScrapeStats::new();
        stats.record_success();
        stats.record_success();
        stats.record_failure();
        stats.record_skip();

        assert_eq!(stats.total, 4);
        assert_eq!(stats.success, 2);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.skipped, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
    }
}
