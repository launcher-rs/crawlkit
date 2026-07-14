//! 表单提取与分析模块
//!
//! 基于 halldyll-parser 的表单提取逻辑改写。
//! 提供从 HTML 文档中提取表单、表单字段、标签关联、表单类型检测等功能。

use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};

use crate::selector::try_parse_selector;

// ============================================================================
// 表单方法
// ============================================================================

/// 表单的 HTTP 方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum FormMethod {
    /// GET 方法
    #[default]
    Get,
    /// POST 方法
    Post,
    /// dialog 方法（HTML5.2）
    Dialog,
}


impl FormMethod {
    /// 从字符串解析表单方法（不区分大小写）
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "post" => Self::Post,
            "dialog" => Self::Dialog,
            _ => Self::Get,
        }
    }
}

// ============================================================================
// 表单类型
// ============================================================================

/// 表单的类型（根据字段和上下文推断）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FormType {
    /// 登录表单
    Login,
    /// 注册表单
    Registration,
    /// 搜索表单
    Search,
    /// 联系表单
    Contact,
    /// 新闻订阅表单
    Newsletter,
    /// 密码重置表单
    PasswordReset,
    /// 结账/支付表单
    Checkout,
    /// 评论表单
    Comment,
    /// 文件上传表单
    Upload,
    /// 未知类型
    Unknown,
}

impl FormType {
    /// 返回所有已知表单类型
    pub fn all() -> &'static [Self] {
        &[
            Self::Login,
            Self::Registration,
            Self::Search,
            Self::Contact,
            Self::Newsletter,
            Self::PasswordReset,
            Self::Checkout,
            Self::Comment,
            Self::Upload,
            Self::Unknown,
        ]
    }
}

// ============================================================================
// 字段类型
// ============================================================================

/// 表单字段的 HTML 输入类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// 纯文本
    Text,
    /// 邮箱
    Email,
    /// 密码
    Password,
    /// 数字
    Number,
    /// 电话
    Tel,
    /// URL
    Url,
    /// 搜索
    Search,
    /// 多行文本域
    Textarea,
    /// 下拉选择
    Select,
    /// 复选框
    Checkbox,
    /// 单选按钮
    Radio,
    /// 文件上传
    File,
    /// 隐藏字段
    Hidden,
    /// 提交按钮
    Submit,
    /// 普通按钮
    Button,
    /// 日期
    Date,
    /// 时间
    Time,
    /// 颜色
    Color,
    /// 范围滑块
    Range,
    /// 未知类型
    Unknown,
}

impl FieldType {
    /// 从 HTML `type` 属性解析字段类型
    pub fn from_html_type(type_attr: Option<&str>) -> Self {
        match type_attr {
            Some(t) => match t.trim().to_lowercase().as_str() {
                "text" => Self::Text,
                "email" => Self::Email,
                "password" => Self::Password,
                "number" => Self::Number,
                "tel" => Self::Tel,
                "url" => Self::Url,
                "search" => Self::Search,
                "checkbox" => Self::Checkbox,
                "radio" => Self::Radio,
                "file" => Self::File,
                "hidden" => Self::Hidden,
                "submit" => Self::Submit,
                "button" => Self::Button,
                "date" => Self::Date,
                "time" => Self::Time,
                "color" => Self::Color,
                "range" => Self::Range,
                _ => Self::Unknown,
            },
            None => Self::Text,
        }
    }

    /// 判断是否为输入值型字段（用户可输入数据的字段）
    pub fn is_input(&self) -> bool {
        matches!(
            self,
            Self::Text
                | Self::Email
                | Self::Password
                | Self::Number
                | Self::Tel
                | Self::Url
                | Self::Search
                | Self::Textarea
                | Self::Date
                | Self::Time
                | Self::Color
                | Self::Range
        )
    }

    /// 判断是否为选择型字段
    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Select | Self::Checkbox | Self::Radio)
    }

    /// 判断是否为按钮型字段
    pub fn is_button(&self) -> bool {
        matches!(self, Self::Submit | Self::Button)
    }
}

// ============================================================================
// 选择选项
// ============================================================================

/// `<select>` 元素的选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    /// 选项值
    pub value: String,
    /// 选项显示文本
    pub label: String,
    /// 是否默认选中
    pub selected: bool,
}

impl SelectOption {
    /// 创建新选项
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            selected: false,
        }
    }
}

// ============================================================================
// 表单字段
// ============================================================================

/// 表单中的单个字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    /// 字段 name 属性
    pub name: Option<String>,
    /// 字段 id 属性
    pub id: Option<String>,
    /// 字段类型
    pub field_type: FieldType,
    /// 关联的 label 文本
    pub label: Option<String>,
    /// placeholder 属性
    pub placeholder: Option<String>,
    /// 默认值
    pub value: Option<String>,
    /// 是否必填
    pub required: bool,
    /// 是否禁用
    pub disabled: bool,
    /// 是否只读
    pub readonly: bool,
    /// autocomplete 属性值
    pub autocomplete: Option<String>,
    /// 输入模式校验正则
    pub pattern: Option<String>,
    /// 最小长度
    pub min_length: Option<usize>,
    /// 最大长度
    pub max_length: Option<usize>,
    /// 选择型字段的选项列表
    pub options: Vec<SelectOption>,
}

impl FormField {
    /// 创建新的表单字段
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: Some(name.into()),
            id: None,
            field_type,
            label: None,
            placeholder: None,
            value: None,
            required: false,
            disabled: false,
            readonly: false,
            autocomplete: None,
            pattern: None,
            min_length: None,
            max_length: None,
            options: Vec::new(),
        }
    }

    /// 创建一个匿名字段（无 name 属性）
    pub fn anonymous(field_type: FieldType) -> Self {
        Self {
            name: None,
            id: None,
            field_type,
            label: None,
            placeholder: None,
            value: None,
            required: false,
            disabled: false,
            readonly: false,
            autocomplete: None,
            pattern: None,
            min_length: None,
            max_length: None,
            options: Vec::new(),
        }
    }
}

// ============================================================================
// 表单
// ============================================================================

/// 提取出的 HTML 表单结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form {
    /// 表单 id 属性
    pub id: Option<String>,
    /// 表单 name 属性
    pub name: Option<String>,
    /// 提交目标 URL（action 属性）
    pub action: Option<String>,
    /// 提交方法
    pub method: FormMethod,
    /// 编码类型
    pub enctype: Option<String>,
    /// 表单中的字段列表
    pub fields: Vec<FormField>,
    /// 推断的表单类型
    pub form_type: FormType,
    /// 是否包含 CSRF 令牌字段
    pub has_csrf: bool,
    /// 是否包含验证码
    pub has_captcha: bool,
    /// 提交按钮文本
    pub submit_text: Option<String>,
}

impl Form {
    /// 创建新表单
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取指定 name 的字段
    pub fn get_field_by_name(&self, name: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.name.as_deref() == Some(name))
    }

    /// 获取所有可见字段（非 hidden 类型）
    pub fn visible_fields(&self) -> Vec<&FormField> {
        self.fields
            .iter()
            .filter(|f| !matches!(f.field_type, FieldType::Hidden))
            .collect()
    }

    /// 获取所有需要用户输入的字段
    pub fn input_fields(&self) -> Vec<&FormField> {
        self.fields.iter().filter(|f| f.field_type.is_input()).collect()
    }
}

impl Default for Form {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            action: None,
            method: FormMethod::default(),
            enctype: None,
            fields: Vec::new(),
            form_type: FormType::Unknown,
            has_csrf: false,
            has_captcha: false,
            submit_text: None,
        }
    }
}

// ============================================================================
// 标签关联
// ============================================================================

/// 将 `<label>` 元素与表单字段关联。
///
/// 通过 `for` 属性匹配字段 id，或通过包裹关系匹配。
/// 返回更新后的字段列表。
pub fn associate_labels(document: &Html, fields: Vec<FormField>) -> Vec<FormField> {
    let label_sel = match try_parse_selector("label") {
        Some(s) => s,
        None => return fields,
    };

    let mut field_map: std::collections::HashMap<String, FormField> = std::collections::HashMap::new();
    let mut unmatched: Vec<FormField> = Vec::new();

    for f in fields {
        if let Some(id) = f.id.clone() {
            field_map.insert(id, f);
        } else {
            unmatched.push(f);
        }
    }

    for label_elem in document.select(&label_sel) {
        let for_attr = label_elem.value().attr("for");
        let label_text: String = label_elem.text().collect::<Vec<_>>().join(" ").trim().to_string();

        if let Some(for_id) = for_attr
            && let Some(field) = field_map.get_mut(for_id) {
                if field.label.is_none() {
                    field.label = Some(label_text);
                }
                continue;
            }

        // 没有 for 属性时，查找 label 包裹的第一个表单控件
        if let Some(child_input) = find_first_form_control(&label_elem) {
            let child_id = child_input.value().attr("id");
            let child_name = child_input.value().attr("name");

            if let Some(id) = child_id
                && let Some(field) = field_map.get_mut(id) {
                    if field.label.is_none() {
                        field.label = Some(label_text);
                    }
                    continue;
                }

            if let Some(name) = child_name
                && let Some(field) = field_map.values_mut().find(|f| f.name.as_deref() == Some(name)) {
                    if field.label.is_none() {
                        field.label = Some(label_text);
                    }
                    continue;
                }
        }
    }

    let mut result: Vec<FormField> = field_map.into_values().collect();
    result.append(&mut unmatched);
    result
}

/// 在元素内查找第一个表单控件子元素
fn find_first_form_control<'a>(element: &'a ElementRef<'a>) -> Option<ElementRef<'a>> {
    for child in element.children() {
        if let Some(child_elem) = ElementRef::wrap(child) {
            let tag = child_elem.value().name();
            if matches!(tag, "input" | "select" | "textarea") {
                return Some(child_elem);
            }
        }
    }
    None
}

// ============================================================================
// 提取输入字段
// ============================================================================

/// 从 `<input>` 元素提取表单字段
pub fn extract_input_field(element: &ElementRef) -> Option<FormField> {
    let tag = element.value().name();
    if tag != "input" {
        return None;
    }

    let type_attr = element.value().attr("type");
    let field_type = FieldType::from_html_type(type_attr);

    let mut field = FormField::anonymous(field_type);

    field.name = element.value().attr("name").map(std::string::ToString::to_string);
    field.id = element.value().attr("id").map(std::string::ToString::to_string);
    field.placeholder = element.value().attr("placeholder").map(std::string::ToString::to_string);
    field.value = element.value().attr("value").map(std::string::ToString::to_string);
    field.required = element.value().attr("required").is_some();
    field.disabled = element.value().attr("disabled").is_some();
    field.readonly = element.value().attr("readonly").is_some();
    field.autocomplete = element.value().attr("autocomplete").map(std::string::ToString::to_string);
    field.pattern = element.value().attr("pattern").map(std::string::ToString::to_string);
    field.min_length = element.value().attr("minlength").and_then(|v| v.parse().ok());
    field.max_length = element.value().attr("maxlength").and_then(|v| v.parse().ok());

    Some(field)
}

/// 从 `<select>` 元素提取表单字段
pub fn extract_select_field(element: &ElementRef) -> Option<FormField> {
    let tag = element.value().name();
    if tag != "select" {
        return None;
    }

    let mut field = FormField::anonymous(FieldType::Select);

    field.name = element.value().attr("name").map(std::string::ToString::to_string);
    field.id = element.value().attr("id").map(std::string::ToString::to_string);
    field.required = element.value().attr("required").is_some();
    field.disabled = element.value().attr("disabled").is_some();

    // 提取选项
    let option_sel = try_parse_selector("option");
    if let Some(ref sel) = option_sel {
        for opt_elem in element.select(sel) {
            let value = opt_elem.value().attr("value").unwrap_or("");
            let label: String = opt_elem.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let label = if label.is_empty() {
                value.to_string()
            } else {
                label
            };
            let selected = opt_elem.value().attr("selected").is_some();
            field.options.push(SelectOption {
                value: value.to_string(),
                label,
                selected,
            });
        }
    }

    Some(field)
}

/// 从 `<textarea>` 元素提取表单字段
pub fn extract_textarea_field(element: &ElementRef) -> Option<FormField> {
    let tag = element.value().name();
    if tag != "textarea" {
        return None;
    }

    let mut field = FormField::anonymous(FieldType::Textarea);

    field.name = element.value().attr("name").map(std::string::ToString::to_string);
    field.id = element.value().attr("id").map(std::string::ToString::to_string);
    field.placeholder = element.value().attr("placeholder").map(std::string::ToString::to_string);
    field.required = element.value().attr("required").is_some();
    field.disabled = element.value().attr("disabled").is_some();
    field.readonly = element.value().attr("readonly").is_some();
    field.value = Some(element.text().collect::<Vec<_>>().join(" ").trim().to_string());
    field.max_length = element.value().attr("maxlength").and_then(|v| v.parse().ok());

    Some(field)
}

/// 从 `<form>` 元素中提取所有表单字段
pub fn extract_form_fields(element: &ElementRef) -> Vec<FormField> {
    let mut fields = Vec::new();

    // 提取 input 元素
    let input_sel = try_parse_selector("input, select, textarea");
    let input_sel = match input_sel {
        Some(s) => s,
        None => return fields,
    };

    for child in element.select(&input_sel) {
        let tag = child.value().name();
        let field = match tag {
            "input" => extract_input_field(&child),
            "select" => extract_select_field(&child),
            "textarea" => extract_textarea_field(&child),
            _ => None,
        };

        if let Some(f) = field {
            fields.push(f);
        }
    }

    fields
}

// ============================================================================
// 表单提取
// ============================================================================

/// 从单个 `<form>` 元素提取 Form 结构
pub fn extract_form(element: &ElementRef) -> Option<Form> {
    let tag = element.value().name();
    if tag != "form" {
        return None;
    }

    let mut form = Form::new();

    form.id = element.value().attr("id").map(std::string::ToString::to_string);
    form.name = element.value().attr("name").map(std::string::ToString::to_string);
    form.action = element.value().attr("action").map(std::string::ToString::to_string);
    form.method = FormMethod::from_str(element.value().attr("method").unwrap_or("get"));
    form.enctype = element.value().attr("enctype").map(std::string::ToString::to_string);

    // 提取字段
    form.fields = extract_form_fields(element);

    // 检测表单特征
    form.has_csrf = detect_csrf_token(&form.fields);
    form.has_captcha = detect_captcha(&form.fields);
    form.form_type = detect_form_type(&form);
    form.submit_text = extract_submit_text(element);

    Some(form)
}

/// 从 HTML 文档中提取所有表单
pub fn extract_forms(document: &Html) -> Vec<Form> {
    let form_sel = match try_parse_selector("form") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut forms = Vec::new();

    for form_elem in document.select(&form_sel) {
        if let Some(form) = extract_form(&form_elem) {
            // 关联 label
            let labeled = associate_labels(document, form.fields);
            forms.push(Form { fields: labeled, ..form });
        }
    }

    forms
}

// ============================================================================
// CSRF 令牌检测
// ============================================================================

/// 检测表单字段中是否包含 CSRF 令牌。
///
/// 通过字段名称中的关键字（csrf、token、_token、authenticity_token 等）判断。
pub fn detect_csrf_token(fields: &[FormField]) -> bool {
    let csrf_patterns = &[
        "csrf",
        "_token",
        "token",
        "authenticity_token",
        "csrf_token",
        "csrfmiddlewaretoken",
        "__csrf",
        "csrfkey",
        "csrf_name",
        "csrf_value",
        "_csrf_token",
        "xsrf",
        "_xsrf",
        "x-csrf-token",
    ];

    fields.iter().any(|field| {
        if matches!(field.field_type, FieldType::Hidden) {
            if let Some(ref name) = field.name {
                let lower = name.to_lowercase();
                csrf_patterns.iter().any(|p| lower.contains(p))
            } else {
                false
            }
        } else {
            false
        }
    })
}

// ============================================================================
// 验证码检测
// ============================================================================

/// 检测表单字段中是否包含验证码。
///
/// 通过字段 name、id、class、placeholder 中的关键字（captcha、recaptcha、g-recaptcha 等）判断。
pub fn detect_captcha(fields: &[FormField]) -> bool {
    let captcha_patterns = &[
        "captcha",
        "recaptcha",
        "g-recaptcha",
        "h-captcha",
        "turnstile",
        "cf-turnstile",
        "captchacode",
        "captcha_code",
        "security_code",
        "verification_code",
        "captcha_id",
        "simple_captcha",
        "math_captcha",
    ];

    fields.iter().any(|field| {
        let name_lower = field.name.as_deref().unwrap_or("").to_lowercase();
        let id_lower = field.id.as_deref().unwrap_or("").to_lowercase();

        captcha_patterns.iter().any(|p| {
            name_lower.contains(p) || id_lower.contains(p)
        })
    })
}

// ============================================================================
// 提交文本提取
// ============================================================================

/// 从 `<form>` 元素中提取提交按钮文本。
///
/// 查找 input[type=submit]、button[type=submit] 的 value 或文本内容。
pub fn extract_submit_text(element: &ElementRef) -> Option<String> {
    let submit_sel = try_parse_selector("input[type=submit], button[type=submit]");
    let submit_sel = submit_sel?;

    for btn in element.select(&submit_sel) {
        let tag = btn.value().name();
        let text = match tag {
            "input" => btn.value().attr("value").map(std::string::ToString::to_string),
            "button" => {
                let t: String = btn.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if t.is_empty() {
                    btn.value().attr("value").map(std::string::ToString::to_string)
                } else {
                    Some(t)
                }
            }
            _ => None,
        };
        if text.is_some() {
            return text;
        }
    }

    None
}

// ============================================================================
// 表单类型检测
// ============================================================================

/// 根据表单的字段特征推断表单类型
pub fn detect_form_type(form: &Form) -> FormType {
    let name_lower = form.name.as_deref().unwrap_or("").to_lowercase();
    let id_lower = form.id.as_deref().unwrap_or("").to_lowercase();
    let action_lower = form.action.as_deref().unwrap_or("").to_lowercase();

    let field_names: Vec<String> = form
        .fields
        .iter()
        .filter_map(|f| f.name.as_ref())
        .map(|n| n.to_lowercase())
        .collect();

    let field_ids: Vec<String> = form
        .fields
        .iter()
        .filter_map(|f| f.id.as_ref())
        .map(|id| id.to_lowercase())
        .collect();

    let has_password = form.fields.iter().any(|f| matches!(f.field_type, FieldType::Password));
    let has_email = form.fields.iter().any(|f| matches!(f.field_type, FieldType::Email));
    let has_file = form.fields.iter().any(|f| matches!(f.field_type, FieldType::File));
    let has_search = form.fields.iter().any(|f| matches!(f.field_type, FieldType::Search));

    // 收集所有可用于匹配的字符串
    let all_text: Vec<&str> = field_names
        .iter()
        .map(std::string::String::as_str)
        .chain(field_ids.iter().map(std::string::String::as_str))
        .chain(std::iter::once(name_lower.as_str()))
        .chain(std::iter::once(id_lower.as_str()))
        .chain(std::iter::once(action_lower.as_str()))
        .collect();

    // 登录表单检测
    if has_password {
        for s in &all_text {
            let lower = s.to_lowercase();
            if lower.contains("login")
                || lower.contains("signin")
                || lower.contains("sign_in")
                || lower.contains("log-in")
                || lower.contains("log_in")
            {
                return FormType::Login;
            }
        }

        // 有密码字段且包含注册关键字 -> 注册
        for s in &all_text {
            let lower = s.to_lowercase();
            if lower.contains("register")
                || lower.contains("signup")
                || lower.contains("sign_up")
                || lower.contains("sign-up")
                || lower.contains("create_account")
                || lower.contains("create-account")
            {
                return FormType::Registration;
            }
        }

        // 有邮件 + 密码 + 记住我 这类典型登录特征
        if has_email
            && field_names.iter().any(|n| n.contains("remember") || n.contains("stay"))
        {
            return FormType::Login;
        }

        // 密码重置检测
        for s in &all_text {
            let lower = s.to_lowercase();
            if lower.contains("password_reset")
                || lower.contains("reset_password")
                || lower.contains("forgot_password")
                || lower.contains("reset-password")
                || lower.contains("forgot-password")
                || lower.contains("recover")
            {
                return FormType::PasswordReset;
            }
        }

        return FormType::Login;
    }

    // 搜索表单检测
    if has_search {
        return FormType::Search;
    }
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("search")
            || lower.contains("query")
            || lower.contains("q=")
            || lower == "q"
            || lower.contains("find")
        {
            return FormType::Search;
        }
    }

    // 文件上传检测
    if has_file {
        return FormType::Upload;
    }
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("upload")
            || lower.contains("file")
            || lower.contains("attachment")
        {
            return FormType::Upload;
        }
    }

    // 联系表单检测
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("contact")
            || lower.contains("feedback")
            || lower.contains("support")
            || lower.contains("inquiry")
            || lower.contains("enquiry")
        {
            return FormType::Contact;
        }
    }

    // 新闻订阅检测
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("newsletter")
            || lower.contains("subscribe")
            || lower.contains("subscription")
            || lower.contains("mailing_list")
        {
            return FormType::Newsletter;
        }
    }

    // 结账检测
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("checkout")
            || lower.contains("payment")
            || lower.contains("cart")
            || lower.contains("billing")
            || lower.contains("order")
        {
            return FormType::Checkout;
        }
    }

    // 评论检测
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("comment")
            || lower.contains("reply")
            || lower.contains("feedback")
        {
            return FormType::Comment;
        }
    }

    // 注册检测（无密码时的备用检测）
    for s in &all_text {
        let lower = s.to_lowercase();
        if lower.contains("register")
            || lower.contains("signup")
            || lower.contains("sign_up")
            || lower.contains("create_account")
            || lower.contains("registration")
        {
            return FormType::Registration;
        }
    }

    FormType::Unknown
}

// ============================================================================
// 便利函数
// ============================================================================

/// 从表单列表中筛选出登录表单
pub fn get_login_forms(forms: &[Form]) -> Vec<&Form> {
    forms.iter().filter(|f| f.form_type == FormType::Login).collect()
}

/// 从表单列表中筛选出搜索表单
pub fn get_search_forms(forms: &[Form]) -> Vec<&Form> {
    forms.iter().filter(|f| f.form_type == FormType::Search).collect()
}

/// 从表单列表中筛选出联系表单
pub fn get_contact_forms(forms: &[Form]) -> Vec<&Form> {
    forms.iter().filter(|f| f.form_type == FormType::Contact).collect()
}

/// 检查 HTML 文档中是否包含表单
pub fn has_forms(document: &Html) -> bool {
    let form_sel = match try_parse_selector("form") {
        Some(s) => s,
        None => return false,
    };
    document.select(&form_sel).next().is_some()
}

/// 检查 HTML 文档中是否包含登录表单
pub fn has_login_form(document: &Html) -> bool {
    let forms = extract_forms(document);
    !get_login_forms(&forms).is_empty()
}

/// 检查 HTML 文档中是否包含搜索表单
pub fn has_search_form(document: &Html) -> bool {
    let forms = extract_forms(document);
    !get_search_forms(&forms).is_empty()
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 辅助函数 ──────────────────────────────────────────

    fn parse_html(html: &str) -> Html {
        Html::parse_document(html)
    }

    fn parse_fragment(html: &str) -> Html {
        Html::parse_fragment(html)
    }

    fn first_form_element(doc: &Html) -> ElementRef<'_> {
        let sel = try_parse_selector("form").unwrap();
        doc.select(&sel).next().expect("应存在表单元素")
    }

    // ─── FormMethod 测试 ──────────────────────────────────────

    #[test]
    fn test_form_method_from_str() {
        assert_eq!(FormMethod::from_str("get"), FormMethod::Get);
        assert_eq!(FormMethod::from_str("GET"), FormMethod::Get);
        assert_eq!(FormMethod::from_str("post"), FormMethod::Post);
        assert_eq!(FormMethod::from_str("POST"), FormMethod::Post);
        assert_eq!(FormMethod::from_str("dialog"), FormMethod::Dialog);
        assert_eq!(FormMethod::from_str(""), FormMethod::Get);
        assert_eq!(FormMethod::from_str("invalid"), FormMethod::Get);
    }

    #[test]
    fn test_form_method_default() {
        let method: FormMethod = Default::default();
        assert_eq!(method, FormMethod::Get);
    }

    #[test]
    fn test_form_method_serialize() {
        let json = serde_json::to_string(&FormMethod::Post).unwrap();
        assert_eq!(json, "\"post\"");
    }

    // ─── FieldType 测试 ──────────────────────────────────────

    #[test]
    fn test_field_type_from_html_type() {
        assert_eq!(FieldType::from_html_type(Some("text")), FieldType::Text);
        assert_eq!(FieldType::from_html_type(Some("email")), FieldType::Email);
        assert_eq!(FieldType::from_html_type(Some("password")), FieldType::Password);
        assert_eq!(FieldType::from_html_type(Some("number")), FieldType::Number);
        assert_eq!(FieldType::from_html_type(Some("checkbox")), FieldType::Checkbox);
        assert_eq!(FieldType::from_html_type(Some("submit")), FieldType::Submit);
        assert_eq!(FieldType::from_html_type(Some("hidden")), FieldType::Hidden);
        assert_eq!(FieldType::from_html_type(Some("file")), FieldType::File);
        assert_eq!(FieldType::from_html_type(Some("date")), FieldType::Date);
        assert_eq!(FieldType::from_html_type(Some("color")), FieldType::Color);
        assert_eq!(FieldType::from_html_type(Some("range")), FieldType::Range);
        assert_eq!(FieldType::from_html_type(Some("tel")), FieldType::Tel);
        assert_eq!(FieldType::from_html_type(Some("url")), FieldType::Url);
        assert_eq!(FieldType::from_html_type(Some("search")), FieldType::Search);
        assert_eq!(FieldType::from_html_type(Some("radio")), FieldType::Radio);
        assert_eq!(FieldType::from_html_type(Some("button")), FieldType::Button);
        assert_eq!(FieldType::from_html_type(Some("time")), FieldType::Time);
        assert_eq!(FieldType::from_html_type(Some("unknown_type")), FieldType::Unknown);
        assert_eq!(FieldType::from_html_type(None), FieldType::Text);
    }

    #[test]
    fn test_field_type_classification() {
        assert!(FieldType::Text.is_input());
        assert!(FieldType::Email.is_input());
        assert!(FieldType::Password.is_input());
        assert!(!FieldType::Hidden.is_input());
        assert!(!FieldType::Submit.is_input());

        assert!(FieldType::Select.is_selectable());
        assert!(FieldType::Checkbox.is_selectable());
        assert!(FieldType::Radio.is_selectable());
        assert!(!FieldType::Text.is_selectable());

        assert!(FieldType::Submit.is_button());
        assert!(FieldType::Button.is_button());
        assert!(!FieldType::Text.is_button());
    }

    // ─── SelectOption 测试 ────────────────────────────────────

    #[test]
    fn test_select_option_creation() {
        let opt = SelectOption::new("cn", "中国");
        assert_eq!(opt.value, "cn");
        assert_eq!(opt.label, "中国");
        assert!(!opt.selected);
    }

    // ─── FormField 测试 ───────────────────────────────────────

    #[test]
    fn test_form_field_creation() {
        let field = FormField::new("username", FieldType::Text);
        assert_eq!(field.name, Some("username".to_string()));
        assert_eq!(field.field_type, FieldType::Text);
        assert!(!field.required);
        assert!(!field.disabled);
    }

    #[test]
    fn test_form_field_anonymous() {
        let field = FormField::anonymous(FieldType::Hidden);
        assert!(field.name.is_none());
        assert_eq!(field.field_type, FieldType::Hidden);
    }

    // ─── 提取 Input 字段测试 ──────────────────────────────────

    #[test]
    fn test_extract_input_field_basic() {
        let html = r#"<input type="text" name="username" id="user" placeholder="请输入用户名" required>"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("input").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_input_field(&elem).unwrap();

        assert_eq!(field.name, Some("username".to_string()));
        assert_eq!(field.id, Some("user".to_string()));
        assert_eq!(field.field_type, FieldType::Text);
        assert_eq!(field.placeholder, Some("请输入用户名".to_string()));
        assert!(field.required);
        assert!(!field.disabled);
    }

    #[test]
    fn test_extract_input_field_hidden() {
        let html = r#"<input type="hidden" name="csrf_token" value="abc123">"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("input").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_input_field(&elem).unwrap();

        assert_eq!(field.field_type, FieldType::Hidden);
        assert_eq!(field.value, Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_input_field_all_attributes() {
        let html = r#"<input type="password" name="pass" id="pwd" placeholder="密码" value="" required disabled readonly autocomplete="off" pattern=".{6,}" minlength="6" maxlength="20">"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("input").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_input_field(&elem).unwrap();

        assert_eq!(field.field_type, FieldType::Password);
        assert_eq!(field.name, Some("pass".to_string()));
        assert_eq!(field.id, Some("pwd".to_string()));
        assert_eq!(field.autocomplete, Some("off".to_string()));
        assert_eq!(field.pattern, Some(".{6,}".to_string()));
        assert_eq!(field.min_length, Some(6));
        assert_eq!(field.max_length, Some(20));
        assert!(field.required);
        assert!(field.disabled);
        assert!(field.readonly);
    }

    #[test]
    fn test_extract_input_field_non_input_tag() {
        let html = r"<span>不是 input</span>";
        let doc = parse_fragment(html);
        let sel = try_parse_selector("span").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let result = extract_input_field(&elem);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_input_field_no_type_defaults_to_text() {
        let html = r#"<input name="q">"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("input").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_input_field(&elem).unwrap();
        assert_eq!(field.field_type, FieldType::Text);
    }

    // ─── 提取 Select 字段测试 ────────────────────────────────

    #[test]
    fn test_extract_select_field_basic() {
        let html = r#"<select name="country" id="country">
            <option value="cn">中国</option>
            <option value="us" selected>美国</option>
            <option value="jp">日本</option>
        </select>"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("select").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_select_field(&elem).unwrap();

        assert_eq!(field.field_type, FieldType::Select);
        assert_eq!(field.name, Some("country".to_string()));
        assert_eq!(field.options.len(), 3);
        assert_eq!(field.options[0].value, "cn");
        assert_eq!(field.options[0].label, "中国");
        assert!(!field.options[0].selected);
        assert!(field.options[1].selected);
    }

    #[test]
    fn test_extract_select_field_non_select_tag() {
        let html = r"<div>不是 select</div>";
        let doc = parse_fragment(html);
        let sel = try_parse_selector("div").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let result = extract_select_field(&elem);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_select_field_empty() {
        let html = r#"<select name="empty"></select>"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("select").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_select_field(&elem).unwrap();
        assert!(field.options.is_empty());
    }

    #[test]
    fn test_extract_select_field_option_without_value_uses_text() {
        let html = r#"<select name="s"><option>请选择</option></select>"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("select").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_select_field(&elem).unwrap();
        assert_eq!(field.options[0].value, "");
        assert_eq!(field.options[0].label, "请选择");
    }

    // ─── 提取 Textarea 字段测试 ──────────────────────────────

    #[test]
    fn test_extract_textarea_field_basic() {
        let html = r#"<textarea name="bio" id="bio" placeholder="自我介绍" maxlength="500">这是内容</textarea>"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("textarea").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_textarea_field(&elem).unwrap();

        assert_eq!(field.field_type, FieldType::Textarea);
        assert_eq!(field.name, Some("bio".to_string()));
        assert_eq!(field.placeholder, Some("自我介绍".to_string()));
        assert_eq!(field.max_length, Some(500));
        assert_eq!(field.value, Some("这是内容".to_string()));
    }

    #[test]
    fn test_extract_textarea_field_non_textarea_tag() {
        let html = r"<p>不是 textarea</p>";
        let doc = parse_fragment(html);
        let sel = try_parse_selector("p").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let result = extract_textarea_field(&elem);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_textarea_field_empty() {
        let html = r#"<textarea name="empty"></textarea>"#;
        let doc = parse_fragment(html);
        let sel = try_parse_selector("textarea").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let field = extract_textarea_field(&elem).unwrap();
        assert_eq!(field.value, Some(String::new()));
    }

    // ─── 表单字段提取测试 ────────────────────────────────────

    #[test]
    fn test_extract_form_fields_mixed() {
        let html = r#"<form>
            <input type="text" name="username">
            <input type="password" name="password">
            <select name="country">
                <option value="cn">中国</option>
            </select>
            <textarea name="bio"></textarea>
            <input type="hidden" name="token" value="xyz">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let fields = extract_form_fields(&form_elem);

        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0].field_type, FieldType::Text);
        assert_eq!(fields[1].field_type, FieldType::Password);
        assert_eq!(fields[2].field_type, FieldType::Select);
        assert_eq!(fields[3].field_type, FieldType::Textarea);
        assert_eq!(fields[4].field_type, FieldType::Hidden);
    }

    #[test]
    fn test_extract_form_fields_no_form_controls() {
        let html = r"<form><p>没有表单控件</p></form>";
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let fields = extract_form_fields(&form_elem);
        assert!(fields.is_empty());
    }

    // ─── 标签关联测试 ────────────────────────────────────────

    #[test]
    fn test_associate_labels_by_for_attr() {
        let html = r#"<form>
            <label for="email">邮箱地址</label>
            <input type="email" id="email" name="email">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let fields = extract_form_fields(&form_elem);
        let labeled = associate_labels(&doc, fields);

        assert_eq!(labeled.len(), 1);
        assert_eq!(labeled[0].label, Some("邮箱地址".to_string()));
    }

    #[test]
    fn test_associate_labels_by_wrapping() {
        let html = r#"<form>
            <label>用户名
                <input type="text" name="username" id="user">
            </label>
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let fields = extract_form_fields(&form_elem);
        let labeled = associate_labels(&doc, fields);

        assert_eq!(labeled.len(), 1);
        assert_eq!(labeled[0].label, Some("用户名".to_string()));
    }

    #[test]
    fn test_associate_labels_no_labels() {
        let html = r#"<form><input type="text" name="q"></form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let fields = extract_form_fields(&form_elem);
        let labeled = associate_labels(&doc, fields);

        assert_eq!(labeled.len(), 1);
        assert!(labeled[0].label.is_none());
    }

    // ─── 表单提取测试 ────────────────────────────────────────

    #[test]
    fn test_extract_forms_single_form() {
        let html = r#"<html><body>
            <form id="f1" name="login" action="/login" method="post">
                <input type="text" name="user">
                <input type="password" name="pass">
                <input type="submit" value="登录">
            </form>
        </body></html>"#;
        let doc = parse_html(html);
        let forms = extract_forms(&doc);

        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].id, Some("f1".to_string()));
        assert_eq!(forms[0].name, Some("login".to_string()));
        assert_eq!(forms[0].action, Some("/login".to_string()));
        assert_eq!(forms[0].method, FormMethod::Post);
    }

    #[test]
    fn test_extract_forms_multiple_forms() {
        let html = r#"<html><body>
            <form id="search" action="/search"><input type="search" name="q"></form>
            <form id="login" action="/login"><input type="password" name="p"></form>
        </body></html>"#;
        let doc = parse_html(html);
        let forms = extract_forms(&doc);

        assert_eq!(forms.len(), 2);
    }

    #[test]
    fn test_extract_forms_no_forms() {
        let html = r"<html><body><p>无表单</p></body></html>";
        let doc = parse_html(html);
        let forms = extract_forms(&doc);
        assert!(forms.is_empty());
    }

    #[test]
    fn test_extract_form_non_form_element() {
        let html = r"<div>不是 form</div>";
        let doc = parse_fragment(html);
        let sel = try_parse_selector("div").unwrap();
        let elem = doc.select(&sel).next().unwrap();
        let result = extract_form(&elem);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_form_default_method() {
        let html = r#"<form action="/search">
            <input type="text" name="q">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.method, FormMethod::Get);
    }

    #[test]
    fn test_extract_form_dialog_method() {
        let html = r#"<form method="dialog"><input type="text" name="x"></form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.method, FormMethod::Dialog);
    }

    // ─── 表单类型检测测试 ────────────────────────────────────

    #[test]
    fn test_detect_form_type_login() {
        let html = r#"<form id="login-form" action="/login" method="post">
            <input type="text" name="username">
            <input type="password" name="password">
            <input type="submit" value="登录">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Login);
    }

    #[test]
    fn test_detect_form_type_search() {
        let html = r#"<form action="/search">
            <input type="search" name="q" placeholder="搜索...">
            <input type="submit" value="搜索">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Search);
    }

    #[test]
    fn test_detect_form_type_search_by_name() {
        let html = r#"<form>
            <input type="text" name="q" placeholder="搜索">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Search);
    }

    #[test]
    fn test_detect_form_type_registration() {
        let html = r#"<form id="register" action="/signup">
            <input type="text" name="username">
            <input type="email" name="email">
            <input type="password" name="password">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Registration);
    }

    #[test]
    fn test_detect_form_type_contact() {
        let html = r#"<form id="contact-form">
            <input type="text" name="name">
            <input type="email" name="email">
            <textarea name="message"></textarea>
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Contact);
    }

    #[test]
    fn test_detect_form_type_newsletter() {
        let html = r#"<form class="newsletter">
            <input type="email" name="subscribe_email">
            <input type="submit" value="订阅">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Newsletter);
    }

    #[test]
    fn test_detect_form_type_checkout() {
        let html = r#"<form id="checkout-form">
            <input type="text" name="card_number">
            <input type="text" name="address">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Checkout);
    }

    #[test]
    fn test_detect_form_type_upload() {
        let html = r#"<form enctype="multipart/form-data">
            <input type="file" name="file">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Upload);
    }

    #[test]
    fn test_detect_form_type_comment() {
        let html = r#"<form class="comment-form">
            <textarea name="comment"></textarea>
            <input type="submit" value="发表评论">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Comment);
    }

    #[test]
    fn test_detect_form_type_password_reset() {
        let html = r#"<form id="reset-password-form">
            <input type="email" name="email">
            <input type="password" name="new_password">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::PasswordReset);
    }

    #[test]
    fn test_detect_form_type_unknown() {
        let html = r#"<form>
            <input type="text" name="data">
            <input type="number" name="count">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Unknown);
    }

    // ─── CSRF 检测测试 ──────────────────────────────────────

    #[test]
    fn test_detect_csrf_token_present() {
        let fields = vec![
            FormField::new("username", FieldType::Text),
            FormField {
                name: Some("csrf_token".to_string()),
                field_type: FieldType::Hidden,
                value: Some("abc123".to_string()),
                ..FormField::anonymous(FieldType::Hidden)
            },
        ];
        assert!(detect_csrf_token(&fields));
    }

    #[test]
    fn test_detect_csrf_token_absent() {
        let fields = vec![
            FormField::new("username", FieldType::Text),
            FormField::new("password", FieldType::Password),
        ];
        assert!(!detect_csrf_token(&fields));
    }

    #[test]
    fn test_detect_csrf_token_non_hidden_ignored() {
        let fields = vec![FormField {
            name: Some("csrf_token".to_string()),
            field_type: FieldType::Text,
            ..FormField::anonymous(FieldType::Text)
        }];
        assert!(!detect_csrf_token(&fields));
    }

    #[test]
    fn test_detect_csrf_token_various_patterns() {
        let names = &[
            "_token",
            "authenticity_token",
            "csrfmiddlewaretoken",
            "_csrf_token",
            "xsrf",
        ];
        for name in names {
            let fields = vec![FormField {
                name: Some(name.to_string()),
                field_type: FieldType::Hidden,
                ..FormField::anonymous(FieldType::Hidden)
            }];
            assert!(detect_csrf_token(&fields), "应检测到 CSRF 字段: {name}");
        }
    }

    // ─── 验证码检测测试 ────────────────────────────────────

    #[test]
    fn test_detect_captcha_by_name() {
        let fields = vec![FormField {
            name: Some("captcha_code".to_string()),
            field_type: FieldType::Text,
            ..FormField::anonymous(FieldType::Text)
        }];
        assert!(detect_captcha(&fields));
    }

    #[test]
    fn test_detect_captcha_by_id() {
        let fields = vec![FormField {
            id: Some("g-recaptcha".to_string()),
            field_type: FieldType::Text,
            ..FormField::anonymous(FieldType::Text)
        }];
        assert!(detect_captcha(&fields));
    }

    #[test]
    fn test_detect_captcha_absent() {
        let fields = vec![FormField::new("username", FieldType::Text)];
        assert!(!detect_captcha(&fields));
    }

    #[test]
    fn test_detect_captcha_recaptcha() {
        let fields = vec![FormField {
            name: Some("g-recaptcha-response".to_string()),
            field_type: FieldType::Text,
            ..FormField::anonymous(FieldType::Text)
        }];
        assert!(detect_captcha(&fields));
    }

    #[test]
    fn test_detect_captcha_turnstile() {
        let fields = vec![FormField {
            id: Some("cf-turnstile".to_string()),
            field_type: FieldType::Text,
            ..FormField::anonymous(FieldType::Text)
        }];
        assert!(detect_captcha(&fields));
    }

    // ─── 提交文本提取测试 ────────────────────────────────────

    #[test]
    fn test_extract_submit_text_input() {
        let html = r#"<form>
            <input type="submit" value="登录">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let text = extract_submit_text(&form_elem);
        assert_eq!(text, Some("登录".to_string()));
    }

    #[test]
    fn test_extract_submit_text_button() {
        let html = r#"<form>
            <button type="submit">注册账号</button>
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let text = extract_submit_text(&form_elem);
        assert_eq!(text, Some("注册账号".to_string()));
    }

    #[test]
    fn test_extract_submit_text_button_with_value() {
        let html = r#"<form>
            <button type="submit" value="提交">发送</button>
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let text = extract_submit_text(&form_elem);
        assert_eq!(text, Some("发送".to_string()));
    }

    #[test]
    fn test_extract_submit_text_no_submit_button() {
        let html = r#"<form><input type="text" name="q"></form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let text = extract_submit_text(&form_elem);
        assert!(text.is_none());
    }

    #[test]
    fn test_extract_submit_text_only_first() {
        let html = r#"<form>
            <input type="submit" value="提交">
            <input type="submit" value="重置">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let text = extract_submit_text(&form_elem);
        assert_eq!(text, Some("提交".to_string()));
    }

    // ─── 便利函数测试 ────────────────────────────────────────

    #[test]
    fn test_get_login_forms() {
        let mut login_form = Form::new();
        login_form.form_type = FormType::Login;
        let mut search_form = Form::new();
        search_form.form_type = FormType::Search;

        let forms = vec![login_form, search_form];
        let logins = get_login_forms(&forms);
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].form_type, FormType::Login);
    }

    #[test]
    fn test_get_search_forms() {
        let mut login_form = Form::new();
        login_form.form_type = FormType::Login;
        let mut search_form = Form::new();
        search_form.form_type = FormType::Search;

        let forms = vec![login_form, search_form];
        let searches = get_search_forms(&forms);
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].form_type, FormType::Search);
    }

    #[test]
    fn test_get_contact_forms() {
        let mut contact_form = Form::new();
        contact_form.form_type = FormType::Contact;
        let mut unknown_form = Form::new();
        unknown_form.form_type = FormType::Unknown;

        let forms = vec![contact_form, unknown_form];
        let contacts = get_contact_forms(&forms);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].form_type, FormType::Contact);
    }

    #[test]
    fn test_get_contact_forms_empty() {
        let forms: Vec<Form> = Vec::new();
        assert!(get_contact_forms(&forms).is_empty());
    }

    #[test]
    fn test_has_forms_true() {
        let html = r#"<html><body><form><input type="text"></form></body></html>"#;
        let doc = parse_html(html);
        assert!(has_forms(&doc));
    }

    #[test]
    fn test_has_forms_false() {
        let html = r"<html><body><p>无表单</p></body></html>";
        let doc = parse_html(html);
        assert!(!has_forms(&doc));
    }

    #[test]
    fn test_has_login_form_true() {
        let html = r#"<html><body>
            <form id="login" action="/login">
                <input type="text" name="username">
                <input type="password" name="password">
            </form>
        </body></html>"#;
        let doc = parse_html(html);
        assert!(has_login_form(&doc));
    }

    #[test]
    fn test_has_login_form_false() {
        let html = r#"<html><body>
            <form action="/search">
                <input type="search" name="q">
            </form>
        </body></html>"#;
        let doc = parse_html(html);
        assert!(!has_login_form(&doc));
    }

    #[test]
    fn test_has_search_form_true() {
        let html = r#"<html><body>
            <form action="/search">
                <input type="search" name="q">
            </form>
        </body></html>"#;
        let doc = parse_html(html);
        assert!(has_search_form(&doc));
    }

    #[test]
    fn test_has_search_form_false() {
        let html = r"<html><body><p>无搜索</p></body></html>";
        let doc = parse_html(html);
        assert!(!has_search_form(&doc));
    }

    // ─── Form 方法测试 ──────────────────────────────────────

    #[test]
    fn test_form_get_field_by_name() {
        let mut form = Form::new();
        form.fields = vec![
            FormField::new("username", FieldType::Text),
            FormField::new("password", FieldType::Password),
        ];
        assert!(form.get_field_by_name("username").is_some());
        assert!(form.get_field_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_form_visible_fields() {
        let mut form = Form::new();
        form.fields = vec![
            FormField::new("username", FieldType::Text),
            FormField {
                name: Some("token".to_string()),
                field_type: FieldType::Hidden,
                ..FormField::anonymous(FieldType::Hidden)
            },
        ];
        let visible = form.visible_fields();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, Some("username".to_string()));
    }

    #[test]
    fn test_form_input_fields() {
        let mut form = Form::new();
        form.fields = vec![
            FormField::new("text", FieldType::Text),
            FormField::new("email", FieldType::Email),
            FormField::new("submit", FieldType::Submit),
            FormField {
                name: Some("hidden".to_string()),
                field_type: FieldType::Hidden,
                ..FormField::anonymous(FieldType::Hidden)
            },
        ];
        let inputs = form.input_fields();
        assert_eq!(inputs.len(), 2);
    }

    // ─── 集成测试 ──────────────────────────────────────────

    #[test]
    fn test_complete_login_form_extraction() {
        let html = r#"<html><body>
            <form id="login-form" action="/signin" method="post" name="login">
                <label for="username">用户名</label>
                <input type="text" id="username" name="username" placeholder="请输入用户名" required>

                <label for="password">密码</label>
                <input type="password" id="password" name="password" placeholder="请输入密码" required minlength="6">

                <label>
                    <input type="checkbox" name="remember" value="1"> 记住我
                </label>

                <input type="hidden" name="_csrf_token" value="abc123">

                <button type="submit">登录</button>
            </form>
        </body></html>"#;

        let doc = parse_html(html);
        let forms = extract_forms(&doc);

        assert_eq!(forms.len(), 1);
        let form = &forms[0];

        assert_eq!(form.id, Some("login-form".to_string()));
        assert_eq!(form.name, Some("login".to_string()));
        assert_eq!(form.action, Some("/signin".to_string()));
        assert_eq!(form.method, FormMethod::Post);
        assert_eq!(form.form_type, FormType::Login);
        assert!(form.has_csrf);
        assert!(!form.has_captcha);
        assert_eq!(form.submit_text, Some("登录".to_string()));

        assert_eq!(form.fields.len(), 4);

        // 验证字段标签关联
        let user_field = form.get_field_by_name("username").unwrap();
        assert_eq!(user_field.label, Some("用户名".to_string()));
        assert!(user_field.required);

        let pass_field = form.get_field_by_name("password").unwrap();
        assert_eq!(pass_field.label, Some("密码".to_string()));
        assert_eq!(pass_field.min_length, Some(6));

        let csrf_field = form.get_field_by_name("_csrf_token").unwrap();
        assert_eq!(csrf_field.field_type, FieldType::Hidden);
    }

    #[test]
    fn test_complete_search_form_extraction() {
        let html = r#"<html><body>
            <form action="/search" method="get" id="search-form">
                <label for="q">搜索</label>
                <input type="search" id="q" name="q" placeholder="输入关键词">
                <button type="submit">搜索</button>
            </form>
        </body></html>"#;

        let doc = parse_html(html);
        let forms = extract_forms(&doc);

        assert_eq!(forms.len(), 1);
        let form = &forms[0];

        assert_eq!(form.form_type, FormType::Search);
        assert_eq!(form.method, FormMethod::Get);
        assert_eq!(form.submit_text, Some("搜索".to_string()));
        assert!(!form.has_csrf);

        let field = form.get_field_by_name("q").unwrap();
        assert_eq!(field.label, Some("搜索".to_string()));
        assert_eq!(field.field_type, FieldType::Search);
    }

    #[test]
    fn test_no_forms_in_empty_document() {
        let html = r"<html><head><title>空页面</title></head><body></body></html>";
        let doc = parse_html(html);
        let forms = extract_forms(&doc);
        assert!(forms.is_empty());
        assert!(!has_forms(&doc));
    }

    #[test]
    fn test_complex_form_with_select_and_textarea() {
        let html = r#"<html><body>
            <form id="contact-form" action="/contact" method="post">
                <label for="name">姓名</label>
                <input type="text" id="name" name="name" required>

                <label for="category">分类</label>
                <select id="category" name="category">
                    <option value="general">一般咨询</option>
                    <option value="support" selected>技术支持</option>
                    <option value="sales">销售</option>
                </select>

                <label for="message">消息</label>
                <textarea id="message" name="message" placeholder="请输入内容" required></textarea>

                <input type="submit" value="提交">
            </form>
        </body></html>"#;

        let doc = parse_html(html);
        let forms = extract_forms(&doc);

        assert_eq!(forms.len(), 1);
        let form = &forms[0];

        assert_eq!(form.form_type, FormType::Contact);
        assert_eq!(form.fields.len(), 4);

        // 检查 select 字段
        let category = form.get_field_by_name("category").unwrap();
        assert_eq!(category.field_type, FieldType::Select);
        assert_eq!(category.options.len(), 3);
        assert_eq!(category.options[0].label, "一般咨询");
        assert!(category.options[1].selected);

        // 检查 textarea 字段
        let message = form.get_field_by_name("message").unwrap();
        assert_eq!(message.field_type, FieldType::Textarea);
        assert!(message.required);
    }

    #[test]
    fn test_form_type_all_enum_variants() {
        let types = FormType::all();
        assert_eq!(types.len(), 10);
        assert!(types.contains(&FormType::Login));
        assert!(types.contains(&FormType::Unknown));
    }

    #[test]
    fn test_form_without_id_or_name() {
        let html = r#"<form action="/submit">
            <input type="text" name="data">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert!(form.id.is_none());
        assert!(form.name.is_none());
        assert_eq!(form.form_type, FormType::Unknown);
    }

    #[test]
    fn test_extract_forms_handles_malformed_html() {
        let html = r#"<html><body>
            <form action="/login">
                <input type=text name=user>
                <input type=password name=pass>
            </form>
        </body></html>"#;
        let doc = parse_html(html);
        let forms = extract_forms(&doc);
        assert_eq!(forms.len(), 1);
    }

    #[test]
    fn test_detect_form_type_signin_variant() {
        let html = r#"<form action="/signin">
            <input type="text" name="user">
            <input type="password" name="pass">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Login);
    }

    #[test]
    fn test_detect_form_type_create_account() {
        let html = r#"<form id="create-account-form">
            <input type="email" name="email">
            <input type="password" name="password">
            <input type="password" name="confirm_password">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Registration);
    }

    #[test]
    fn test_detect_form_type_search_with_q_name() {
        let html = r#"<form>
            <input type="text" name="q">
        </form>"#;
        let doc = parse_html(html);
        let form_elem = first_form_element(&doc);
        let form = extract_form(&form_elem).unwrap();
        assert_eq!(form.form_type, FormType::Search);
    }

    #[test]
    fn test_detect_csrf_token_django_style() {
        let fields = vec![FormField {
            name: Some("csrfmiddlewaretoken".to_string()),
            field_type: FieldType::Hidden,
            ..FormField::anonymous(FieldType::Hidden)
        }];
        assert!(detect_csrf_token(&fields));
    }

    #[test]
    fn test_detect_csrf_token_no_hidden_skip() {
        let fields = vec![FormField::new("_token", FieldType::Text)];
        assert!(!detect_csrf_token(&fields));
    }

    #[test]
    fn test_detect_captcha_hcaptcha() {
        let fields = vec![FormField {
            id: Some("h-captcha".to_string()),
            field_type: FieldType::Text,
            ..FormField::anonymous(FieldType::Text)
        }];
        assert!(detect_captcha(&fields));
    }

    #[test]
    fn test_get_login_forms_no_match() {
        let forms = vec![
            Form { form_type: FormType::Search, ..Form::new() },
            Form { form_type: FormType::Contact, ..Form::new() },
        ];
        assert!(get_login_forms(&forms).is_empty());
    }

    #[test]
    fn test_get_search_forms_multiple() {
        let forms = vec![
            Form { form_type: FormType::Search, ..Form::new() },
            Form { form_type: FormType::Search, ..Form::new() },
            Form { form_type: FormType::Login, ..Form::new() },
        ];
        assert_eq!(get_search_forms(&forms).len(), 2);
    }
}
