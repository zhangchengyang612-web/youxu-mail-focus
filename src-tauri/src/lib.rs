mod classifier;
mod db;
mod ispace_provider;
mod mail_provider;
mod models;
mod reminder_provider;

use crate::mail_provider::{ImapMailProvider, MailProvider};
use crate::models::{AssignmentDeadline, CalendarEvent, ClassificationRule, MailMessage, MailSettings, PersonalReminderInput, ReminderDraft, SystemReminderInput};
use crate::reminder_provider::{ReminderProvider, SystemReminderProvider};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{Manager, State};

struct AppState { db: Mutex<Connection> }

const PASSWORD_SERVICE: &str = "com.mailfocus.wecom.imap";
const ISPACE_SERVICE: &str = "com.mailfocus.wecom.ispace-calendar";
const ISPACE_ACCOUNT: &str = "calendar-feed";

#[cfg(target_os = "macos")]
fn save_secret(service: &str, account: &str, secret: &str) -> Result<(), String> {
    security_framework::passwords::set_generic_password(
        service,
        account,
        secret.as_bytes(),
    )
    .map_err(|error| format!("无法写入 macOS 钥匙串：{error}"))?;

    // A successful return is not enough: verify that a later sync can read the
    // same credential before persisting the non-secret mailbox settings.
    let stored = security_framework::passwords::get_generic_password(service, account)
        .map_err(|error| format!("钥匙串写入后无法读取：{error}"))?;
    if stored != secret.as_bytes() {
        return Err("钥匙串写入验证失败，请检查系统钥匙串权限".into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn load_secret(service: &str, account: &str) -> Result<String, String> {
    let bytes = security_framework::passwords::get_generic_password(service, account)
        .map_err(|_| "系统钥匙串中没有所需凭据，请重新保存设置".to_string())?;
    String::from_utf8(bytes).map_err(|_| "钥匙串中的凭据格式无效，请重新保存设置".into())
}

#[cfg(target_os = "macos")]
fn delete_secret(service: &str, account: &str) -> Result<(), String> {
    security_framework::passwords::delete_generic_password(service, account)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn secret_entry(service: &str, account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(service, account).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn save_secret(service: &str, account: &str, secret: &str) -> Result<(), String> {
    let entry = secret_entry(service, account)?;
    entry.set_password(secret).map_err(|error| format!("无法写入 Windows 凭据管理器：{error}"))?;
    let stored = entry.get_password().map_err(|error| format!("凭据写入后无法读取：{error}"))?;
    if stored != secret { return Err("凭据写入验证失败，请检查系统凭据权限".into()); }
    Ok(())
}

#[cfg(target_os = "windows")]
fn load_secret(service: &str, account: &str) -> Result<String, String> {
    secret_entry(service, account)?.get_password()
        .map_err(|error| format!("系统凭据中没有可用的邮箱密码（{error}），请重新保存设置"))
}

#[cfg(target_os = "windows")]
fn delete_secret(service: &str, account: &str) -> Result<(), String> {
    secret_entry(service, account)?.delete_credential().map_err(|error| error.to_string())
}

#[tauri::command]
fn list_mails(state: State<AppState>) -> Result<Vec<MailMessage>, String> { db::list_mails(&*state.db.lock().map_err(|_| "数据库锁异常")?) }

#[tauri::command]
fn save_mail_settings(state: State<AppState>, mut settings: MailSettings, password: String) -> Result<(), String> {
    if settings.host.trim().is_empty() || settings.email.trim().is_empty() || settings.port == 0 { return Err("邮箱地址、服务器和端口不能为空".into()); }
    if password.is_empty() { return Err("邮箱密码不能为空".into()); }
    settings.email = settings.email.trim().to_string();
    settings.host = settings.host.trim().to_string();
    save_secret(PASSWORD_SERVICE, &settings.email, &password)?;
    let connection = state.db.lock().map_err(|_| "数据库锁异常")?;
    db::save_settings(&connection, &settings)?;
    db::reset_sync_from_now(&connection)
}

#[tauri::command]
fn test_mail_connection(settings: MailSettings, password: String) -> Result<String, String> { ImapMailProvider.test_connection(&settings, &password) }

#[tauri::command]
fn sync_mail(state: State<AppState>) -> Result<usize, String> {
    let settings = { db::load_settings(&*state.db.lock().map_err(|_| "数据库锁异常")?)? };
    let password = load_secret(PASSWORD_SERVICE, settings.email.trim())?;
    let cursor = db::load_sync_cursor(&*state.db.lock().map_err(|_| "数据库锁异常")?)?;
    if cursor.is_none() {
        let baseline = ImapMailProvider.current_uid(&settings, &password)?;
        db::save_sync_cursor(&*state.db.lock().map_err(|_| "数据库锁异常")?, baseline)?;
        return Ok(0);
    }
    let batch = ImapMailProvider.sync_after(&settings, &password, cursor.unwrap())?;
    let connection = state.db.lock().map_err(|_| "数据库锁异常")?;
    let rules = db::list_rules(&connection)?;
    for mail in &batch.mails { let (category, reason) = classifier::classify(mail, &rules); db::upsert_mail(&connection, mail, &category, &reason)?; }
    db::save_sync_cursor(&connection, batch.last_uid)?;
    Ok(batch.mails.len())
}

#[tauri::command]
fn update_mail_category(state: State<AppState>, mail_id: String, category: String) -> Result<(), String> {
    const VALID: &[&str] = &["待办","学业","校园事务","社团活动","个人","外部"];
    if !VALID.contains(&category.as_str()) { return Err("未知分类".into()); }
    db::update_category(&*state.db.lock().map_err(|_| "数据库锁异常")?, &mail_id, &category)
}

#[tauri::command]
fn create_reminder(state: State<AppState>, draft: ReminderDraft) -> Result<String, String> {
    if draft.title.trim().is_empty() { return Err("提醒标题不能为空".into()); }
    if db::reminder_exists(&*state.db.lock().map_err(|_| "数据库锁异常")?, &draft.source_mail_id)? { return Err("这封邮件已经创建过提醒".into()); }
    let (platform, id) = SystemReminderProvider.create(&draft)?;
    db::save_created_reminder(&mut *state.db.lock().map_err(|_| "数据库锁异常")?, &draft, &platform, &id)?;
    Ok(id)
}

#[tauri::command]
fn create_system_reminder(state: State<AppState>, input: SystemReminderInput) -> Result<String, String> {
    if input.source_id.trim().is_empty() { return Err("提醒来源不能为空".into()); }
    if input.title.trim().is_empty() { return Err("提醒标题不能为空".into()); }
    if !matches!(input.priority.as_str(), "low" | "normal" | "high") { return Err("提醒优先级无效".into()); }
    chrono::DateTime::parse_from_rfc3339(&input.due_at).map_err(|_| "提醒时间格式无效".to_string())?;
    if db::system_reminder_exists(&*state.db.lock().map_err(|_| "数据库锁异常")?, &input.source_id)? { return Err("这个事项已经加入过电脑提醒".into()); }
    let draft = ReminderDraft { title: input.title, notes: input.notes, due_at: Some(input.due_at), priority: input.priority, source_mail_id: input.source_id.clone(), source_url: None };
    let (platform, id) = SystemReminderProvider.create(&draft)?;
    db::save_system_reminder_link(&*state.db.lock().map_err(|_| "数据库锁异常")?, &input.source_id, &platform, &id)?;
    Ok(id)
}

#[tauri::command]
fn list_system_reminder_sources(state: State<AppState>) -> Result<Vec<String>, String> {
    db::list_system_reminder_sources(&*state.db.lock().map_err(|_| "数据库锁异常")?)
}

#[tauri::command]
fn list_calendar_events(state: State<AppState>) -> Result<Vec<CalendarEvent>, String> {
    db::list_calendar_events(&*state.db.lock().map_err(|_| "数据库锁异常")?)
}

#[tauri::command]
fn save_personal_reminder(state: State<AppState>, input: PersonalReminderInput) -> Result<CalendarEvent, String> {
    if input.title.trim().is_empty() { return Err("提醒标题不能为空".into()); }
    if !matches!(input.priority.as_str(), "low" | "normal" | "high") { return Err("提醒优先级无效".into()); }
    chrono::DateTime::parse_from_rfc3339(&input.starts_at).map_err(|_| "提醒时间格式无效".to_string())?;
    if input.id.as_deref().is_some_and(|id| !id.starts_with("personal:")) { return Err("只能编辑个人提醒".into()); }
    db::save_personal_event(&*state.db.lock().map_err(|_| "数据库锁异常")?, &input)
}

#[tauri::command]
fn delete_personal_reminder(state: State<AppState>, event_id: String) -> Result<(), String> {
    db::delete_personal_event(&*state.db.lock().map_err(|_| "数据库锁异常")?, &event_id)
}

#[tauri::command]
fn clear_local_data(state: State<AppState>) -> Result<(), String> {
    if let Ok(settings) = db::load_settings(&*state.db.lock().map_err(|_| "数据库锁异常")?) { let _ = delete_secret(PASSWORD_SERVICE, settings.email.trim()); }
    let _ = delete_secret(ISPACE_SERVICE, ISPACE_ACCOUNT);
    db::clear(&*state.db.lock().map_err(|_| "数据库锁异常")?)
}

#[tauri::command]
fn save_ispace_calendar_url(calendar_url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(calendar_url.trim()).map_err(|_| "iSpace 日历 URL 格式无效".to_string())?;
    let trusted = parsed.scheme() == "https" && parsed.host_str().is_some_and(|host| host == "ispace.bnbu.edu.cn" || host.ends_with(".bnbu.edu.cn"));
    if !trusted { return Err("请粘贴由 BNBU iSpace 生成的 HTTPS 日历 URL".into()); }
    save_secret(ISPACE_SERVICE, ISPACE_ACCOUNT, calendar_url.trim())
}

#[tauri::command]
fn list_assignments(state: State<AppState>) -> Result<Vec<AssignmentDeadline>, String> {
    db::list_assignments(&*state.db.lock().map_err(|_| "数据库锁异常")?)
}

#[tauri::command]
fn sync_assignments(state: State<AppState>) -> Result<usize, String> {
    let calendar_url = load_secret(ISPACE_SERVICE, ISPACE_ACCOUNT)
        .map_err(|_| "尚未配置 iSpace 日历订阅 URL".to_string())?;
    let assignments = ispace_provider::fetch_assignments(&calendar_url)?;
    db::replace_assignments(&*state.db.lock().map_err(|_| "数据库锁异常")?, &assignments)?;
    Ok(assignments.len())
}

#[tauri::command]
fn list_rules(state: State<AppState>) -> Result<Vec<ClassificationRule>, String> { db::list_rules(&*state.db.lock().map_err(|_| "数据库锁异常")?) }

#[tauri::command]
fn save_rule(state: State<AppState>, rule: ClassificationRule) -> Result<(), String> {
    const CATEGORIES: &[&str] = &["待办","学业","校园事务","社团活动","个人","外部"];
    const FIELDS: &[&str] = &["sender","domain","subject","body"];
    const OPERATORS: &[&str] = &["contains","equals","regex"];
    if !CATEGORIES.contains(&rule.category.as_str()) || !FIELDS.contains(&rule.field.as_str()) || !OPERATORS.contains(&rule.operator.as_str()) || rule.value.trim().is_empty() { return Err("分类规则参数无效".into()); }
    db::save_rule(&*state.db.lock().map_err(|_| "数据库锁异常")?, &rule)
}

#[tauri::command]
fn delete_rule(state: State<AppState>, rule_id: String) -> Result<(), String> { db::delete_rule(&*state.db.lock().map_err(|_| "数据库锁异常")?, &rule_id) }

pub fn run() {
    tauri::Builder::default().plugin(tauri_plugin_opener::init()).setup(|app| {
        let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&data_dir)?;
        app.manage(AppState { db: Mutex::new(db::open(&data_dir.join("mail-focus.sqlite")).map_err(std::io::Error::other)?) });
        Ok(())
    }).invoke_handler(tauri::generate_handler![list_mails,save_mail_settings,test_mail_connection,sync_mail,update_mail_category,create_reminder,create_system_reminder,list_system_reminder_sources,list_calendar_events,save_personal_reminder,delete_personal_reminder,clear_local_data,list_rules,save_rule,delete_rule,save_ispace_calendar_url,list_assignments,sync_assignments]).run(tauri::generate_context!()).expect("failed to run application");
}
