//! HTTP 响应封装
//!
//! 统一的响应结构体 [`Response`]，屏蔽底层 HTTP 客户端差异，
//! 并提供 `is_bot_challenge()` / `is_access_denied()` 等语义化检测方法。

use std::collections::HashMap;

/// 统一的 HTTP 响应结构
///
/// 无论底层使用 reqwest、wreq 还是 Chrome，都统一转换为该类型。
///
/// # 拦截检测
///
/// `Response` 提供两个核心检测方法，用于识别反爬服务返回的伪成功/拒绝响应：
///
/// - [`is_bot_challenge()`](Response::is_bot_challenge) — 检测 HTTP 200 但内容为机器人验证页面
/// - [`is_access_denied()`](Response::is_access_denied) — 检测 HTTP 403/401 等访问被拒绝页面
///
/// 两者可叠加使用，也可通过 `CompositeFetcher` / `Collector` 的自定义回调扩展检测逻辑。
#[derive(Debug, Clone)]
pub struct Response {
    /// 最终请求的 URL（可能经过重定向）
    pub url: String,
    /// HTTP 状态码
    pub status: u16,
    /// 响应头（key 全小写）
    pub headers: HashMap<String, String>,
    /// 响应体（文本）
    pub body: String,
}

impl Response {
    /// 状态码是否为 2xx
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// 获取 Content-Type（原始值）
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("content-type").map(String::as_str)
    }

    /// 是否为 HTML 内容
    ///
    /// 判断依据（任一命中即为 `true`）：
    /// 1. `Content-Type` 包含 `text/html` 或 `application/xhtml+xml`
    /// 2. body 前 64 字节以 `<!doctype html` 或 `<html` 开头
    pub fn is_html(&self) -> bool {
        // 优先通过 Content-Type 判断
        if let Some(ct) = self.content_type() {
            let ct_lower = ct.to_ascii_lowercase();
            if ct_lower.contains("text/html") || ct_lower.contains("application/xhtml+xml") {
                return true;
            }
        }
        // 降级：检查 body 开头（避免读取整个大文件）
        let trimmed = self.body.trim_start();
        let lower = trimmed[..trimmed.len().min(64)].to_ascii_lowercase();
        lower.starts_with("<!doctype html") || lower.starts_with("<html")
    }

    /// 检测响应是否为机器人验证页面（Bot Challenge）
    ///
    /// 很多反爬服务（PerimeterX、Cloudflare、DataDome、Akamai 等）在检测到爬虫时
    /// 返回 HTTP 200，但 body 内容是 CAPTCHA/challenge 页面，不是真实内容。
    /// 此方法通过检测 body 中的特征关键词来识别这类伪成功响应。
    ///
    /// # 支持的反爬服务
    ///
    /// | 服务 | 检测特征 |
    /// |------|----------|
    /// | PerimeterX / HUMAN Security | `_pxhd`, `_pxuuid`, `px-captcha`, `px-cloud.net` 等 |
    /// | Cloudflare | `cf-challenge`, `challenge-platform`, `ray id` + `cloudflare` |
    /// | DataDome | `datadome`, `captcha-delivery` |
    /// | Akamai Bot Manager | `akamai` + `bot`/`challenge` |
    /// | 通用 | `access to this page` + `denied` + `captcha`/`human` 等 |
    ///
    /// # 示例
    ///
    /// ```rust
    /// use crawlkit_core::response::Response;
    /// use std::collections::HashMap;
    ///
    /// let resp = Response {
    ///     url: "https://example.com".into(),
    ///     status: 200,
    ///     headers: HashMap::from([("content-type".into(), "text/html".into())]),
    ///     body: r#"<html><body>
    ///         <script>window._pxUuid = "abc123";</script>
    ///     </body></html>"#.into(),
    /// };
    /// assert!(resp.is_bot_challenge());
    /// ```
    pub fn is_bot_challenge(&self) -> bool {
        // 仅检测 HTTP 200 的"伪成功"响应
        // 4xx 等错误响应由 is_access_denied() 处理
        if self.status != 200 {
            return false;
        }
        // 非 HTML 内容不可能是验证页面
        if !self.is_html() {
            return false;
        }
        let body_lower = self.body.to_ascii_lowercase();

        // ── PerimeterX / HUMAN Security ──
        // 特征：页面中注入 _px* 系列 JS 变量或引用 px-cloud.net 域名
        if body_lower.contains("_pxhd")
            || body_lower.contains("_pxuuid")
            || body_lower.contains("px-captcha")
            || body_lower.contains("pxcaptcha")
            || body_lower.contains("_pxappappid")
            || body_lower.contains("px-cloud.net")
            || body_lower.contains("humansecurity.com")
        {
            return true;
        }

        // ── Cloudflare challenge ──
        // 特征：cf-challenge div / challenge-platform 脚本 / ray id + cloudflare + enable javascript
        if body_lower.contains("cf-challenge")
            || body_lower.contains("challenge-platform")
            || body_lower.contains("ray id")
                && body_lower.contains("cloudflare")
                && body_lower.contains("enable javascript")
        {
            return true;
        }

        // ── DataDome ──
        // 特征：datadome 域名或 captcha-delivery.com 引用
        if body_lower.contains("datadome") || body_lower.contains("captcha-delivery") {
            return true;
        }

        // ── Akamai Bot Manager ──
        // 特征：页面中明确提及 akamai 且包含 bot/challenge 关键词
        if body_lower.contains("akamai")
            && (body_lower.contains("bot") || body_lower.contains("challenge"))
        {
            return true;
        }

        // ── 通用 challenge 关键词组合 ──
        // 需同时命中多个关键词以减少误判：标题 + 拒绝 + 验证相关词
        if body_lower.contains("access to this page")
            && body_lower.contains("denied")
            && (body_lower.contains("captcha")
                || body_lower.contains("human")
                || body_lower.contains("press & hold")
                || body_lower.contains("verify you are"))
        {
            return true;
        }

        // ── 常见验证页面 title ──
        // 匹配 <title> 标签中的拦截关键词，但要求 body 中同时包含 challenge 以避免误判
        if (body_lower.contains("<title>")
            && (body_lower.contains("attention required")
                || body_lower.contains("access denied")
                || body_lower.contains("please verify")
                || body_lower.contains("security check")
                || body_lower.contains("robot")
                || body_lower.contains("blocked")))
            && body_lower.contains("challenge")
        {
            return true;
        }

        false
    }

    /// 检测响应是否为访问被拒绝页面（Access Denied）
    ///
    /// 很多 CDN/WAF（Akamai、Cloudflare、AWS WAF 等）在拦截爬虫时
    /// 直接返回 HTTP 403，body 为拒绝访问页面，而非真实内容。
    /// 此方法通过状态码和 body 特征来识别这类响应。
    ///
    /// # 检测策略
    ///
    /// 1. **前置过滤**：仅检测 4xx 错误响应（主要是403），且 body 为 HTML 内容
    /// 2. **Akamai CDN**：检测 `edgesuite.net`/`edgekey.net` 域名 + 拒绝关键词
    /// 3. **Akamai Reference #**：短页面 + `Reference #` + `errors.edgesuite.net`
    /// 4. **Cloudflare 403**：状态码403 + `cloudflare` + `denied`/`error 1020`
    /// 5. **通用短页面拒绝**：403 + 短 body（< 3KB）+ `Access Denied`/`403 Forbidden` 等
    ///
    /// # 误判控制
    ///
    /// - 非 HTML 响应（如 JSON API 的403）不检测
    /// - 长 body（> 3KB）的403不检测（可能是正常的业务错误页面）
    /// - 仅 4xx 状态码触发（200/5xx 等不进入此逻辑）
    ///
    /// # 示例
    ///
    /// ```rust
    /// use crawlkit_core::response::Response;
    /// use std::collections::HashMap;
    ///
    /// // Akamai CDN 拒绝页面
    /// let resp = Response {
    ///     url: "https://example.com".into(),
    ///     status: 403,
    ///     headers: HashMap::from([("content-type".into(), "text/html".into())]),
    ///     body: r#"<HTML><HEAD><TITLE>Access Denied</TITLE></HEAD>
    ///     <BODY><H1>Access Denied</H1>
    ///     You don't have permission to access this server.
    ///     Reference #18.5a0ed217.1784187749.21deb4da
    ///     <P>https://errors.edgesuite.net/18.5a0ed217</P>
    ///     </BODY></HTML>"#.into(),
    /// };
    /// assert!(resp.is_access_denied());
    /// ```
    pub fn is_access_denied(&self) -> bool {
        // ── 前置过滤 ──
        // 只检测 4xx 错误响应（主要是 403）
        if !(400..500).contains(&self.status) {
            return false;
        }
        // 非 HTML 内容不检测（可能是正常的 API 403 响应）
        if !self.is_html() {
            return false;
        }
        let body_lower = self.body.to_ascii_lowercase();
        let body_len = self.body.len();

        // ── Akamai CDN 拒绝页面 ──
        // 特征：edgesuite.net / edgekey.net 域名 + Access Denied / You don't have permission
        if body_lower.contains("edgesuite.net") || body_lower.contains("edgekey.net") {
            if body_lower.contains("access denied")
                || body_lower.contains("you don't have permission")
            {
                return true;
            }
        }

        // ── Akamai Reference # 格式 ──
        // 特征：短页面 + Reference # + errors.edgesuite.net / errors.edgekey.net
        // 这是 Akamai CDN 的标准拒绝响应格式
        if body_len < 5000 && body_lower.contains("reference #") {
            if body_lower.contains("errors.edgesuite.net")
                || body_lower.contains("errors.edgekey.net")
            {
                return true;
            }
        }

        // ── Cloudflare 拒绝访问 ──
        // 特征：HTTP 403 + cloudflare 关键词 + denied / error 1020
        // 注意：Cloudflare 的 HTTP 200 challenge 由 is_bot_challenge() 处理
        if self.status == 403
            && body_lower.contains("cloudflare")
            && (body_lower.contains("denied") || body_lower.contains("error 1020"))
        {
            return true;
        }

        // ── 通用短页面拒绝 ──
        // 特征：403 + 短页面（< 3KB）+ 拒绝关键词
        // 长页面的403通常是正常业务逻辑返回的错误页面，不视为拦截
        if body_len < 3000 && self.status == 403 {
            if body_lower.contains("access denied")
                || body_lower.contains("403 forbidden")
                || body_lower.contains("you don't have permission")
                || body_lower.contains("not allowed")
            {
                return true;
            }
        }

        false
    }
}

// ─────────────────────────────────────────────────────────────
// 单元测试
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：快速构建测试用 Response
    fn make_response(status: u16, content_type: &str, body: &str) -> Response {
        Response {
            url: "http://test.com".to_string(),
            status,
            headers: HashMap::from([("content-type".into(), content_type.into())]),
            body: body.to_string(),
        }
    }

    // ════════════════════════════════════════
    //  is_bot_challenge 测试
    // ════════════════════════════════════════

    #[test]
    fn bot_challenge_perimeterx_pxuuid() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><head><title>Captcha</title></head><body>
<script>window._pxUuid = "abc";</script></body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_perimeterx_pxhd() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body><script>var _pxhd = "xyz";</script></body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_cloudflare_cf_challenge() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body>
<div id="cf-challenge">Verifying...</div>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_cloudflare_ray_id() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body>
<p>Ray ID: 7a1b2c3d4e</p>
<p>cloudflare</p>
<p>enable javascript</p>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_datadome() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body>
<script src="https://datadome.co/tag.js"></script>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_captcha_delivery() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body>
<iframe src="https://captcha-delivery.com/block"></iframe>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_akamai_with_bot() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body>
<p>Akamai Bot Manager detected automated traffic</p>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_generic_access_to_page() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><body>
<h1>Access to this page has been denied</h1>
<p>Please verify you are human</p>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_title_with_challenge_keyword() {
        let resp = make_response(
            200,
            "text/html",
            r#"<html><head>
<title>Attention Required</title>
</head><body>
<div class="challenge">Complete the captcha</div>
</body></html>"#,
        );
        assert!(resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_not_triggered_for_normal_200() {
        let resp = make_response(200, "text/html", "<html><body>Hello world</body></html>");
        assert!(!resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_not_triggered_for_non_html() {
        let resp = make_response(200, "application/json", r#"{"_pxhd":"abc"}"#);
        assert!(!resp.is_bot_challenge());
    }

    #[test]
    fn bot_challenge_not_triggered_for_403() {
        // 403 响应不进入 bot challenge 检测（应由 is_access_denied 处理）
        let resp = make_response(403, "text/html", "<html><body>_pxhd test</body></html>");
        assert!(!resp.is_bot_challenge());
    }

    // ════════════════════════════════════════
    //  is_access_denied 测试
    // ════════════════════════════════════════

    #[test]
    fn access_denied_akamai_dhs_gov_example() {
        // 用户提供的 DHS.gov 实际案例
        let resp = make_response(
            403,
            "text/html",
            r#"<HTML><HEAD>
<TITLE>Access Denied</TITLE>
</HEAD><BODY>
<H1>Access Denied</H1>

You don't have permission to access "http://www.dhs.gov/all-news-updates" on this server.<P>
Reference #18.5a0ed217.1784187749.21deb4da
<P>https://errors.edgesuite.net/18.5a0ed217.1784187749.21deb4da</P>
</BODY>
</HTML>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_akamai_edgekey() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><head><title>Access Denied</title></head><body>
<H1>Access Denied</H1>
Reference #123.abc
https://errors.edgekey.net/123.abc
</body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_akamai_with_permission_text() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><body>
You don't have permission to access this resource on this server.
Reference #999.aaa
https://errors.edgesuite.net/999.aaa
</body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_cloudflare_403_error_1020() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><body>
<h1>Error 1020</h1>
<p>Access denied</p>
<div class="cf-error-details">Ray ID: abc123</div>
</body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_cloudflare_denied() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><body>
<div class="cf-wrapper">
<h1>Access denied</h1>
<p>You have been blocked by Cloudflare security.</p>
</div>
</body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_short_generic_403_forbidden() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><head><title>403 Forbidden</title></head>
<body><h1>403 Forbidden</h1></body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_short_generic_access_denied() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><head><title>Access Denied</title></head>
<body><h1>Access Denied</h1></body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_you_dont_have_permission() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><body>
You don't have permission to access this resource.
</body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    #[test]
    fn access_denied_not_allowed() {
        let resp = make_response(
            403,
            "text/html",
            r#"<html><body>
<h1>Not Allowed</h1>
<p>Your request is not allowed by the server.</p>
</body></html>"#,
        );
        assert!(resp.is_access_denied());
    }

    // ── 不触发的场景 ──

    #[test]
    fn access_denied_not_triggered_for_200() {
        let resp = make_response(200, "text/html", "<html><body>Access Denied</body></html>");
        assert!(!resp.is_access_denied());
    }

    #[test]
    fn access_denied_not_triggered_for_long_403() {
        // 长页面的403不误判（可能是正常业务逻辑返回的错误页面）
        let long_body = format!("<html><body>{}</body></html>", "x".repeat(4000));
        let resp = make_response(403, "text/html", &long_body);
        assert!(!resp.is_access_denied());
    }

    #[test]
    fn access_denied_not_triggered_for_non_html() {
        let resp = make_response(403, "application/json", r#"{"error":"forbidden"}"#);
        assert!(!resp.is_access_denied());
    }

    #[test]
    fn access_denied_not_triggered_for_500() {
        let resp = make_response(
            500,
            "text/html",
            "<html><body>Access Denied</body></html>",
        );
        assert!(!resp.is_access_denied());
    }

    #[test]
    fn access_denied_not_triggered_for_401_without_html() {
        let resp = make_response(401, "text/plain", "Unauthorized");
        assert!(!resp.is_access_denied());
    }

    // ════════════════════════════════════════
    //  is_html 测试
    // ════════════════════════════════════════

    #[test]
    fn is_html_by_content_type() {
        let resp = make_response(200, "text/html; charset=utf-8", "plain text");
        assert!(resp.is_html());
    }

    #[test]
    fn is_html_by_body_start() {
        let resp = make_response(200, "text/plain", "<!DOCTYPE html><html><body></body></html>");
        assert!(resp.is_html());
    }

    #[test]
    fn is_not_html() {
        let resp = make_response(200, "application/json", r#"{"key":"value"}"#);
        assert!(!resp.is_html());
    }
}
