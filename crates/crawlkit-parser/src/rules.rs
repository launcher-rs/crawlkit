use regex::Regex;

/// URL 匹配规则，用于判断一个链接是否为文章/新闻链接。
///
/// 内部维护两套正则模式列表：
/// - **包含模式（include）**：命中任意一条即视为文章链接
/// - **排除模式（exclude）**：命中任意一条即跳过，优先级高于包含模式
///
/// 默认内置 30+ 条常用文章路径规则（日期路径、关键词、CMS 模式等），
/// 覆盖绝大多数政府、智库、新闻类站点。对于规则未能覆盖的站点，
/// 可通过链式调用 `with_include` / `with_exclude` 扩展。
///
/// # 示例
///
/// ```rust
/// use crawlkit_parser::UrlRule;
///
/// let rule = UrlRule::default();
///
/// // 日期路径匹配
/// assert!(rule.is_article_url("https://example.com/2024/01/15/hello-world"));
/// // 关键词路径匹配
/// assert!(rule.is_article_url("https://example.com/news/breaking-story"));
/// // 静态资源被排除
/// assert!(!rule.is_article_url("https://example.com/image.jpg"));
/// // 标签页面被排除
/// assert!(!rule.is_article_url("https://example.com/tag/rust"));
///
/// // 扩展自定义规则
/// let rule = UrlRule::default()
///     .with_include(r"/my-section/")
///     .with_exclude(r"print=true")
///     .with_selector("div.content a");
/// assert!(rule.is_article_url("https://example.com/my-section/foo"));
/// assert!(!rule.is_article_url("https://example.com/my-section/foo?print=true"));
/// ```
#[derive(Debug, Clone)]
pub struct UrlRule {
    #[allow(dead_code)]
    pub name: String,
    /// 包含模式列表 —— 命中任意一条即为文章链接
    pub include_patterns: Vec<Regex>,
    /// 排除模式列表 —— 命中任意一条即跳过
    pub exclude_patterns: Vec<Regex>,
    /// 可选的 CSS 选择器，限制只在页面特定区域内查找链接（如 "div.news-list a"）
    pub link_selector: Option<String>,
}

#[allow(dead_code)]
impl UrlRule {
    /// 判断一个 URL 是否符合文章规则。
    ///
    /// 先检查所有排除模式，任一匹配则返回 `false`；
    /// 再检查所有包含模式，任一匹配则返回 `true`；
    /// 均不匹配则返回 `false`。
    ///
    /// ```rust
    /// use crawlkit_parser::UrlRule;
    ///
    /// let rule = UrlRule::default();
    /// assert!(rule.is_article_url("https://example.com/news/story"));
    /// assert!(!rule.is_article_url("https://example.com/style.css"));
    /// assert!(!rule.is_article_url("https://example.com/login"));
    /// ```
    pub fn is_article_url(&self, url: &str) -> bool {
        for ex in &self.exclude_patterns {
            if ex.is_match(url) {
                return false;
            }
        }
        for pat in &self.include_patterns {
            if pat.is_match(url) {
                return true;
            }
        }
        false
    }

    /// 添加一条包含模式（链式调用）。
    ///
    /// ```rust
    /// use crawlkit_parser::UrlRule;
    ///
    /// let rule = UrlRule::default()
    ///     .with_include(r"/custom-section/");
    /// assert!(rule.is_article_url("https://example.com/custom-section/foo"));
    /// ```
    pub fn with_include(mut self, pattern: &str) -> Self {
        self.include_patterns.push(Regex::new(pattern).expect("invalid regex"));
        self
    }

    /// 添加一条排除模式（链式调用）。
    ///
    /// ```rust
    /// use crawlkit_parser::UrlRule;
    ///
    /// let rule = UrlRule::default()
    ///     .with_exclude(r"print=true");
    /// assert!(!rule.is_article_url("https://example.com/news/story?print=true"));
    /// ```
    pub fn with_exclude(mut self, pattern: &str) -> Self {
        self.exclude_patterns.push(Regex::new(pattern).expect("invalid regex"));
        self
    }

    /// 设置 CSS 选择器，限定链接查找范围（链式调用）。
    ///
    /// 设置后 `Extractor` 仅在匹配该选择器的元素内部查找 `<a>` 标签。
    ///
    /// ```rust
    /// use crawlkit_parser::UrlRule;
    ///
    /// let rule = UrlRule::default()
    ///     .with_selector("div.article-list");
    /// assert_eq!(rule.link_selector.as_deref(), Some("div.article-list"));
    /// ```
    pub fn with_selector(mut self, selector: &str) -> Self {
        self.link_selector = Some(selector.to_string());
        self
    }
}

fn default_include_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"/\d{4}/\d{2}/\d{2}/").unwrap(),
        Regex::new(r"/\d{4}/\d{2}/").unwrap(),
        Regex::new(r"\.(html|shtml|htm|cfm|asp|aspx|php)(?:\?|$)").unwrap(),
        Regex::new(r"(?i)/news").unwrap(),
        Regex::new(r"(?i)/articles?/").unwrap(),
        Regex::new(r"(?i)/story/").unwrap(),
        Regex::new(r"(?i)/blog").unwrap(),
        Regex::new(r"(?i)/post").unwrap(),
        Regex::new(r"(?i)/press").unwrap(),
        Regex::new(r"(?i)/releases?/").unwrap(),
        Regex::new(r"(?i)/hearings?").unwrap(),
        Regex::new(r"(?i)/events?/").unwrap(),
        Regex::new(r"(?i)/video/").unwrap(),
        Regex::new(r"(?i)/media/").unwrap(),
        Regex::new(r"(?i)/download/").unwrap(),
        Regex::new(r"(?i)/document/").unwrap(),
        Regex::new(r"(?i)/publication").unwrap(),
        Regex::new(r"(?i)/research/").unwrap(),
        Regex::new(r"(?i)/analysis/").unwrap(),
        Regex::new(r"(?i)/commentary/").unwrap(),
        Regex::new(r"(?i)/report").unwrap(),
        Regex::new(r"(?i)/briefing").unwrap(),
        Regex::new(r"(?i)/archives/").unwrap(),
        Regex::new(r"(?i)/content/").unwrap(),
        Regex::new(r"(?i)/detail/").unwrap(),
        Regex::new(r"(?i)/show/").unwrap(),
        Regex::new(r"(?i)/highlights?/").unwrap(),
        Regex::new(r"(?i)/statuses/").unwrap(),
        Regex::new(r"/p/\d+").unwrap(),
        Regex::new(r"/a/\d+").unwrap(),
        Regex::new(r"(?i)/node/").unwrap(),
        Regex::new(r"(?i)/opa/").unwrap(),
        Regex::new(r"(?i)/news-events/").unwrap(),
        Regex::new(r"(?i)/profiles/").unwrap(),
    ]
}

fn default_exclude_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"\.(css|js|json|xml|rss|ico|png|jpg|jpeg|gif|svg|webp|pdf|zip|rar|exe|dmg|mp4|mp3)$").unwrap(),
        Regex::new(r"(?i)/(tag|tags|category|categories|author|page|login|register|logout|search|wp-content|wp-includes)/").unwrap(),
        Regex::new(r"(javascript:|mailto:|tel:)").unwrap(),
        Regex::new(r"#comment").unwrap(),
    ]
}

impl Default for UrlRule {
    fn default() -> Self {
        Self {
            name: "default".into(),
            include_patterns: default_include_patterns(),
            exclude_patterns: default_exclude_patterns(),
            link_selector: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typical_article_urls() {
        let rule = UrlRule::default();
        let cases = vec![
            "https://example.com/2024/01/15/hello-world",
            "https://example.com/news/breaking-story",
            "https://example.com/article/some-title",
            "https://example.com/story/something",
            "https://example.com/2024/01/15/hello-world.html",
            "https://example.com/p/12345",
            "https://example.com/a/67890",
            "https://example.com/archives/123",
            "https://example.com/detail/xyz",
            "https://example.com/content/abc",
            "https://example.com/show/abc",

        ];
        for url in cases {
            assert!(rule.is_article_url(url), "should match: {url}");
        }
    }

    #[test]
    fn test_non_article_urls() {
        let rule = UrlRule::default();
        let cases = vec![
            "https://example.com/style.css",
            "https://example.com/app.js",
            "https://example.com/image.jpg",
            "https://example.com/tag/rust",
            "https://example.com/categories/tech",
            "https://example.com/login",
            "https://example.com/register",
            "https://example.com/search?q=news",
            "javascript:void(0)",
            "mailto:test@example.com",
        ];
        for url in cases {
            assert!(!rule.is_article_url(url), "should reject: {url}");
        }
    }

    #[test]
    fn test_real_world_unmatched_urls() {
        let rule = UrlRule::default();
        let cases = vec![
            "https://www.heritage.org/progressivism/commentary/the-state-escape",
            "https://www.hoover.org/research/irans-nightmares",
            "https://www.csis.org/analysis/great-sudan-policy-reset",
            "https://www.cnas.org/publications/reports/russia-china-relations",
            "https://fas.org/publication/talent-pipeline-for-the-clean-energy-transition/",
            "https://www.nextgov.com/defense/2023/08/army-gets-new-prototypes/389037/",
            "https://www.defense.gov/News/Releases/Release/Article/3496391/",
            "https://www.justice.gov/opa/pr/final-defendant-sentenced",
            "https://www.fcc.gov/document/cgb-announces-second-round-acp",
            "https://www.transportation.gov/briefing-room/icymi-usdot-launches",
            "https://www.state.gov/reports/country-reports-on-terrorism-2021/",
            "https://www.finance.senate.gov/hearings/open-executive-session",
            "https://www.judiciary.senate.gov/press/releases/durbin-statement",
            "https://energycommerce.house.gov/posts/chair-rodgers-statement",
            "https://trumpstruth.org/statuses/32029",
            "https://www.darpa.mil/news-events/2023-08-15",
            "https://cset.georgetown.edu/publication/china-science-ethics-guiding/",
            "https://www.navy.mil/Press-Office/Press-Briefings/Article/3357084/",
            "https://www.dol.gov/newsroom/releases/eta/eta20230817-0",
            "https://foreignaffairs.house.gov/press-release/mccaul-requests-interview/",
        ];
        for url in cases {
            assert!(rule.is_article_url(url), "should match: {url}");
        }
    }

    #[test]
    fn test_new_patterns_coverage() {
        let rule = UrlRule::default();
        let cases = vec![
            "https://www.energy.gov/articles/biden-harris-announces",
            "https://cyberscoop.com/video/understanding-the-economic-impact/",
            "https://cyberscoop.com/event/microsoft-federal-innovation-series/",
            "https://www.stimson.org/event/taiwans-economic-security/",
            "https://www.atlanticcouncil.org/in-depth-research-reports/report/sanctioning-china/",
            "https://www.disa.mil/en/NewsandEvents/2023/AbilityOne-Base-Supply-Center",
            "https://science.osti.gov/bes/Highlights/2023/BES-2023-08-b",
            "https://www.finance.senate.gov/download/122222-letter",
            "https://www.afcea.org/signal/resources/linkreq.cfm?id=491",
            "https://www.usda.gov/media/radio/daily-newsline/2023-08-16/actuality-opportunities",
        ];
        for url in cases {
            assert!(rule.is_article_url(url), "should match: {url}");
        }
    }

    #[test]
    fn test_year_month_without_day() {
        let rule = UrlRule::default();
        assert!(rule.is_article_url("https://example.com/2024/03/some-article"));
        assert!(rule.is_article_url("https://example.com/2023/08/another-one"));
    }

    #[test]
    fn test_with_include_extension() {
        let rule = UrlRule::default();
        assert!(rule.is_article_url("https://example.com/blog/my-post"));
    }

    #[test]
    fn test_with_exclude_override() {
        let rule = UrlRule::default().with_exclude(r"print=true");
        assert!(!rule.is_article_url("https://example.com/news/foo?print=true"));
    }
}
