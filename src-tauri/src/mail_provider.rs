use crate::models::{MailSettings, ParsedMail};
use chrono::Utc;
use mailparse::{parse_mail, MailHeaderMap, ParsedMail as MimeMail};
use native_tls::TlsConnector;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub struct SyncBatch {
    pub mails: Vec<ParsedMail>,
    pub last_uid: u32,
}

pub trait MailProvider {
    fn test_connection(&self, settings: &MailSettings, password: &str) -> Result<String, String>;
    fn current_uid(&self, settings: &MailSettings, password: &str) -> Result<u32, String>;
    fn sync_after(&self, settings: &MailSettings, password: &str, after_uid: u32) -> Result<SyncBatch, String>;
}

pub struct ImapMailProvider;

impl ImapMailProvider {
    fn connect(&self, settings: &MailSettings, password: &str) -> Result<imap::Session<native_tls::TlsStream<std::net::TcpStream>>, String> {
        let tls = TlsConnector::builder().build().map_err(|e| format!("TLS 初始化失败：{e}"))?;
        let addresses = (settings.host.as_str(), settings.port).to_socket_addrs().map_err(|e| format!("无法解析 IMAP 地址：{e}"))?;
        let mut tcp = None;
        for address in addresses {
            if let Ok(stream) = TcpStream::connect_timeout(&address, Duration::from_secs(15)) {
                tcp = Some(stream);
                break;
            }
        }
        let tcp = tcp.ok_or_else(|| "连接 IMAP 超时，请检查网络后重试".to_string())?;
        tcp.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
        tcp.set_write_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
        let stream = tls.connect(&settings.host, tcp).map_err(|e| format!("TLS 连接失败：{e}"))?;
        let mut client = imap::Client::new(stream);
        client.read_greeting().map_err(|e| format!("IMAP 服务器无响应：{e}"))?;
        client.login(&settings.email, password).map_err(|(e, _)| format!("邮箱认证失败：{e}"))
    }
}

impl MailProvider for ImapMailProvider {
    fn test_connection(&self, settings: &MailSettings, password: &str) -> Result<String, String> {
        let mut session = self.connect(settings, password)?;
        let mailbox = session.select("INBOX").map_err(|e| format!("无法打开收件箱：{e}"))?;
        let _ = session.logout();
        Ok(format!("连接成功，收件箱共 {} 封邮件", mailbox.exists))
    }

    fn current_uid(&self, settings: &MailSettings, password: &str) -> Result<u32, String> {
        let mut session = self.connect(settings, password)?;
        let mailbox = session.select("INBOX").map_err(|e| format!("无法打开收件箱：{e}"))?;
        let baseline = mailbox.uid_next.unwrap_or(1).saturating_sub(1);
        let _ = session.logout();
        Ok(baseline)
    }

    fn sync_after(&self, settings: &MailSettings, password: &str, after_uid: u32) -> Result<SyncBatch, String> {
        let mut session = self.connect(settings, password)?;
        session.select("INBOX").map_err(|e| format!("无法打开收件箱：{e}"))?;
        let query = format!("UID {}:*", after_uid.saturating_add(1));
        let ids = session.uid_search(&query).map_err(|e| format!("搜索新邮件失败：{e}"))?;
        let mut sorted: Vec<u32> = ids.into_iter().filter(|uid| *uid > after_uid).collect();
        sorted.sort_unstable();
        let last_uid = sorted.last().copied().unwrap_or(after_uid);
        let mut output = Vec::new();
        for chunk in sorted.chunks(25) {
            let set = chunk.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
            // Limit each message to 256 KiB and request header/text only. This
            // prevents large attachments from blocking the UI or entering SQLite.
            let fetched = session.uid_fetch(&set, "(UID FLAGS BODY.PEEK[HEADER] BODY.PEEK[TEXT]<0.262144>)").map_err(|e| format!("读取新邮件失败：{e}"))?;
            for item in fetched.iter() {
                if let (Some(uid), Some(header)) = (item.uid, item.header()) {
                    let mut raw = header.to_vec();
                    raw.extend_from_slice(b"\r\n");
                    if let Some(text) = item.text() { raw.extend_from_slice(text); }
                    if let Ok(parsed) = parse_message(uid, &raw, item.flags().iter().any(|f| matches!(f, &imap::types::Flag::Seen))) { output.push(parsed); }
                }
            }
        }
        let _ = session.logout();
        Ok(SyncBatch { mails: output, last_uid })
    }
}

fn parse_message(uid: u32, raw: &[u8], is_read: bool) -> Result<ParsedMail, String> {
    let parsed = parse_mail(raw).map_err(|e| e.to_string())?;
    let from = parsed.headers.get_first_value("From").unwrap_or_default();
    let (sender_name, sender_email) = split_address(&from);
    let recipients = parsed.headers.get_first_value("To").unwrap_or_default().split(',').map(|v| split_address(v).1).filter(|v| !v.is_empty()).collect();
    let received_at = parsed.headers.get_first_value("Date").and_then(|v| mailparse::dateparse(&v).ok()).and_then(|ts| chrono::DateTime::from_timestamp(ts,0)).unwrap_or_else(Utc::now).to_rfc3339();
    Ok(ParsedMail { uid, sender_name, sender_email, recipients, subject: parsed.headers.get_first_value("Subject").unwrap_or_else(|| "（无主题）".into()), received_at, body_text: normalize_body_text(extract_text(&parsed)), is_read })
}

pub(crate) fn normalize_body_text(text: String) -> String {
    text.replace("&amp;nbsp;", " ")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&#xA0;", " ")
        .replace("&#xa0;", " ")
        .replace('\u{00a0}', " ")
}

fn extract_text(mail: &MimeMail<'_>) -> String {
    if mail.get_content_disposition().disposition == mailparse::DispositionType::Attachment {
        return String::new();
    }
    if !mail.subparts.is_empty() && !mail.ctype.mimetype.starts_with("multipart/") {
        return String::new();
    }
    if mail.subparts.is_empty() {
        if !mail.ctype.mimetype.starts_with("text/") { return String::new(); }
        let body = mail.get_body().unwrap_or_default();
        if mail.ctype.mimetype.eq_ignore_ascii_case("text/html") { return html2text::from_read(body.as_bytes(), 100); }
        return body;
    }
    if let Some(part) = mail.subparts.iter().find(|p| p.ctype.mimetype.eq_ignore_ascii_case("text/plain")) { return part.get_body().unwrap_or_default(); }
    mail.subparts.iter().map(extract_text).filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

fn split_address(value: &str) -> (String, String) {
    if let (Some(start), Some(end)) = (value.rfind('<'), value.rfind('>')) {
        return (value[..start].trim().trim_matches('"').to_string(), value[start+1..end].trim().to_string());
    }
    let email = value.trim().to_string(); (email.clone(), email)
}

#[cfg(test)]
mod tests {
    use super::normalize_body_text;

    #[test]
    fn decodes_non_breaking_space_entities_in_mail_text() {
        let text = normalize_body_text("·&nbsp; A ·&amp;nbsp; B ·&#160; C ·&#xA0; 16:00。".into());
        assert_eq!(text, "·  A ·  B ·  C ·  16:00。");
    }
}
