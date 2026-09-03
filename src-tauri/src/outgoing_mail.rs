use crate::models::{MailSettings, OutgoingMail, ProfessorContact};
use base64::{engine::general_purpose::STANDARD, Engine};
use lettre::message::{header::ContentType, Attachment, Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;

const SMTP_HOST: &str = "smtp.exmail.qq.com";
const SMTP_PORT: u16 = 465;
const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
const STAFF_DIRECTORY_API: &str = "https://staff.bnbu.edu.cn/teacher/teacher/list";

pub fn send(settings: &MailSettings, sender_name: &str, password: &str, outgoing: OutgoingMail) -> Result<String, String> {
    if outgoing.to.is_empty() { return Err("请至少填写一位收件人".into()); }
    if outgoing.subject.trim().is_empty() { return Err("请填写邮件主题".into()); }
    if outgoing.text_body.trim().is_empty() && outgoing.html_body.trim().is_empty() { return Err("请填写邮件正文".into()); }

    let address = settings.email.parse().map_err(|_| "发件邮箱格式无效".to_string())?;
    let from = Mailbox::new((!sender_name.trim().is_empty()).then(|| sender_name.trim().to_string()), address);
    let mut builder = Message::builder().from(from).subject(outgoing.subject.trim());
    for address in &outgoing.to { builder = builder.to(parse_mailbox(address)?); }
    for address in &outgoing.cc { builder = builder.cc(parse_mailbox(address)?); }
    for address in &outgoing.bcc { builder = builder.bcc(parse_mailbox(address)?); }

    let total = outgoing.attachments.iter().try_fold(0usize, |sum, item| {
        if item.name.trim().is_empty() { return Err("附件名称不能为空".to_string()); }
        let bytes = STANDARD.decode(&item.data_base64).map_err(|_| format!("附件 {} 数据无效", item.name))?;
        sum.checked_add(bytes.len()).ok_or_else(|| "附件总大小无效".to_string())
    })?;
    if total > MAX_ATTACHMENT_BYTES { return Err("附件总大小不能超过 20 MB".into()); }

    let html = safe_editor_html(&outgoing.html_body);
    let text = if outgoing.text_body.trim().is_empty() { html2text::from_read(html.as_bytes(), 100) } else { outgoing.text_body };
    let (text, html) = append_signature(text, html, sender_name);
    let alternative = MultiPart::alternative_plain_html(text, html);
    let mut mixed = MultiPart::mixed().multipart(alternative);
    for item in outgoing.attachments {
        let bytes = STANDARD.decode(&item.data_base64).map_err(|_| format!("附件 {} 数据无效", item.name))?;
        let content_type = item.mime_type.parse::<ContentType>().unwrap_or(ContentType::parse("application/octet-stream").expect("static mime"));
        mixed = mixed.singlepart(Attachment::new(item.name).body(bytes, content_type));
    }
    let message = builder.multipart(mixed).map_err(|error| format!("邮件内容无效：{error}"))?;
    let mailer = SmtpTransport::relay(SMTP_HOST)
        .map_err(|error| format!("SMTP TLS 初始化失败：{error}"))?
        .port(SMTP_PORT)
        .credentials(Credentials::new(settings.email.clone(), password.to_string()))
        .timeout(Some(Duration::from_secs(40)))
        .build();
    let response = mailer.send(&message).map_err(|error| format!("发送失败：{error}"))?;
    Ok(format!("发送成功 · {}", response.code()))
}

pub fn translate_english_to_chinese(text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() { return Err("邮件正文为空".into()); }
    if text.chars().count() > 12_000 { return Err("邮件正文过长，请展开后选择需要翻译的部分".into()); }
    let client = Client::builder().timeout(Duration::from_secs(25)).user_agent("MailFocus-BNBU/0.6").build().map_err(|e| e.to_string())?;
    let mut translated = Vec::new();
    for chunk in split_utf8_chunks(text, 450) {
        let response: Value = client.get("https://api.mymemory.translated.net/get")
            .query(&[("q", chunk), ("langpair", "en|zh-CN")])
            .send().and_then(|response| response.error_for_status())
            .map_err(|error| format!("在线翻译失败：{error}"))?
            .json().map_err(|error| format!("翻译服务返回格式无效：{error}"))?;
        let value = response.pointer("/responseData/translatedText").and_then(Value::as_str).unwrap_or_default().trim();
        if value.is_empty() { return Err("翻译服务没有返回译文".into()); }
        translated.push(value.to_string());
    }
    Ok(translated.join("\n"))
}

fn split_utf8_chunks(value: &str, max_bytes: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while value.len().saturating_sub(start) > max_bytes {
        let mut end = start;
        let mut boundary = None;
        for (offset, character) in value[start..].char_indices() {
            if offset > max_bytes { break; }
            end = start + offset + character.len_utf8();
            if character.is_whitespace() || matches!(character, '.' | '!' | '?' | ';' | ':' | ',' | '。' | '！' | '？') { boundary = Some(end); }
        }
        let cut = boundary.filter(|value| *value > start).unwrap_or(end);
        chunks.push(value[start..cut].trim());
        start = cut;
    }
    if start < value.len() { chunks.push(value[start..].trim()); }
    chunks.into_iter().filter(|chunk| !chunk.is_empty()).collect()
}

fn parse_mailbox(value: &str) -> Result<Mailbox, String> {
    value.trim().parse().map_err(|_| format!("邮箱地址格式无效：{value}"))
}

fn safe_editor_html(value: &str) -> String {
    let blocked = Regex::new(r"(?is)<\s*(?:script|iframe|object|embed|style)[^>]*>.*?<\s*/\s*(?:script|iframe|object|embed|style)\s*>").expect("static regex");
    let events = Regex::new(r#"(?i)\s+on[a-z]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)"#).expect("static regex");
    events.replace_all(&blocked.replace_all(value, ""), "").into_owned()
}

fn append_signature(text: String, html: String, sender_name: &str) -> (String, String) {
    let name = sender_name.trim();
    if name.is_empty() { return (text, html); }
    let escaped_name = name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    (
        format!("{}\n\n北师港浸大 BNBU\n{}", text.trim_end(), name),
        format!("{}<div style=\"margin-top:32px;border-top:1px solid #e5e7eb;padding-top:14px;color:#4b5563\"><div>北师港浸大 BNBU</div><strong>{escaped_name}</strong></div>", html.trim_end()),
    )
}

pub fn search_professors(query: &str) -> Result<Vec<ProfessorContact>, String> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 60 { return Err("请输入教授姓名的一部分".into()); }
    let client = Client::builder().timeout(Duration::from_secs(20)).user_agent("MailFocus-BNBU/0.6").build().map_err(|e| e.to_string())?;
    let response: Value = client.get(STAFF_DIRECTORY_API)
        .query(&[("lang", "cn"), ("page", "0"), ("pageSize", "100"), ("key", query)])
        .send().and_then(|response| response.error_for_status())
        .map_err(|error| format!("暂时无法连接 BNBU 教师名录：{error}"))?
        .json().map_err(|_| "BNBU 教师名录返回格式无效".to_string())?;
    if response.get("code").and_then(Value::as_i64) != Some(0) { return Err("BNBU 教师名录查询失败，请稍后重试".into()); }
    let mut contacts = parse_staff_directory(&response, query);
    contacts.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    contacts.truncate(50);
    Ok(contacts)
}

fn parse_staff_directory(response: &Value, query: &str) -> Vec<ProfessorContact> {
    let needle = query.to_lowercase();
    let mut seen = HashSet::new();
    response.pointer("/data/data").and_then(Value::as_array).into_iter().flatten().filter_map(|item| {
        let chinese_name = item.get("name").and_then(Value::as_str).unwrap_or_default().trim();
        let english_name = item.get("name_en").and_then(Value::as_str).unwrap_or_default().trim();
        let email = item.get("email").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase();
        let haystack = format!("{chinese_name} {english_name} {email}").to_lowercase();
        if email.is_empty() || !email.ends_with("@bnbu.edu.cn") || !haystack.contains(&needle) || !seen.insert(email.clone()) { return None; }
        let name = match (chinese_name.is_empty(), english_name.is_empty()) {
            (false, false) => format!("{chinese_name} · {english_name}"),
            (false, true) => chinese_name.to_string(),
            (true, false) => english_name.to_string(),
            (true, true) => email.split('@').next().unwrap_or_default().to_string(),
        };
        let title_cn = item.pointer("/teacher_title/title").and_then(Value::as_str).unwrap_or_default().trim();
        let title_en = item.pointer("/teacher_title/title_en").and_then(Value::as_str).unwrap_or_default().trim();
        let department = match (title_cn.is_empty(), title_en.is_empty()) {
            (false, false) => format!("{title_cn} · {title_en}"),
            (false, true) => title_cn.to_string(),
            (true, false) => title_en.to_string(),
            (true, true) => "BNBU 教师".into(),
        };
        let username = item.get("username").and_then(Value::as_str).unwrap_or_default();
        let source_url = if username.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')) && !username.is_empty() {
            format!("https://www.bnbu.edu.cn/faculty.htm#/{username}/cn")
        } else {
            "https://www.bnbu.edu.cn/faculty.htm#/list/people/cn.html".into()
        };
        Some(ProfessorContact { name, email, department, source_url })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_partial_names_from_the_official_staff_directory() {
        let response = serde_json::json!({"data":{"data":[
            {"name":"周小焱","name_en":"Xiaoyan ZHOU","email":"xiaoyanzhou@bnbu.edu.cn","username":"xiaoyanzhou","teacher_title":{"title":"助理教授","title_en":"Assistant Professor"}},
            {"name":"王鹏","name_en":"Peng WANG","email":"pengwang@bnbu.edu.cn","username":"pengwang","teacher_title":{"title":"教授","title_en":"Professor"}}
        ]}});
        let contacts = parse_staff_directory(&response, "xiaoyan");
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "周小焱 · Xiaoyan ZHOU");
        assert_eq!(contacts[0].email, "xiaoyanzhou@bnbu.edu.cn");
        assert!(contacts[0].source_url.ends_with("#/xiaoyanzhou/cn"));
    }

    #[test]
    #[ignore = "calls the live BNBU staff directory"]
    fn live_partial_name_search_finds_xiaoyan_zhou() {
        let contacts = search_professors("xiaoyan").expect("search live directory");
        assert!(contacts.iter().any(|contact| contact.email == "xiaoyanzhou@bnbu.edu.cn"));
    }

    #[test]
    fn splits_translation_text_without_breaking_utf8() {
        let chunks = split_utf8_chunks("Hello 世界, this is a longer sentence.", 10);
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat().replace(' ', ""), "Hello世界,thisisalongersentence.");
    }

    #[test]
    fn appends_the_bnbu_signature_to_both_mail_formats() {
        let (text, html) = append_signature("Hello".into(), "<p>Hello</p>".into(), "Student Name");
        assert!(text.ends_with("北师港浸大 BNBU\nStudent Name"));
        assert!(html.contains("北师港浸大 BNBU"));
        assert!(html.contains("Student Name"));
    }
}
