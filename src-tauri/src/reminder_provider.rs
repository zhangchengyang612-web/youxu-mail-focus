use crate::models::ReminderDraft;
use std::process::Command;
use uuid::Uuid;

pub trait ReminderProvider { fn create(&self, draft: &ReminderDraft) -> Result<(String, String), String>; }
pub struct SystemReminderProvider;

impl ReminderProvider for SystemReminderProvider {
    fn create(&self, draft: &ReminderDraft) -> Result<(String, String), String> {
        #[cfg(target_os = "macos")]
        {
            // Apple Reminders presents the native privacy prompt on first use.
            let script = r#"function run(argv) {
  const reminderTitle = argv[0];
  const reminderNotes = argv[1];
  const dueText = argv[2];
  let dueDate = null;
  if (dueText) {
    dueDate = new Date(dueText);
    if (Number.isNaN(dueDate.getTime())) throw new Error("invalid reminder date");
  }
  const reminders = Application("Reminders");
  const properties = {name: reminderTitle, body: reminderNotes};
  if (dueDate) properties.dueDate = dueDate;
  const newReminder = reminders.Reminder(properties);
  reminders.defaultList().reminders.push(newReminder);
  return newReminder.id();
}"#;
            let due = validate_due_date(draft.due_at.as_deref())?.unwrap_or_default();
            let out = Command::new("osascript").args(["-l", "JavaScript", "-e", script, &draft.title, &draft.notes, due]).output().map_err(|e| format!("无法启动提醒事项：{e}"))?;
            if !out.status.success() { return Err(format!("提醒事项拒绝创建：{}", String::from_utf8_lossy(&out.stderr))); }
            let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return Ok(("apple-reminders".into(), if id.is_empty() { Uuid::new_v4().to_string() } else { id }));
        }
        #[cfg(target_os = "windows")]
        {
            // The production adapter uses OAuth PKCE and Microsoft Graph Tasks.ReadWrite.
            // A tenant/client id must be supplied before a distributable Windows build.
            let _ = draft;
            return Err("Windows 版需要先在设置中配置 Microsoft Entra Client ID 并完成 To Do 授权".into());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        { let _ = draft; Err("当前平台暂不支持系统提醒".into()) }
    }
}

fn validate_due_date(value: Option<&str>) -> Result<Option<&str>, String> {
    let Some(value) = value else { return Ok(None); };
    chrono::DateTime::parse_from_rfc3339(value).map_err(|_| "提醒时间格式无效".to_string())?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_iso_due_date_without_locale_parsing() {
        assert_eq!(validate_due_date(Some("2026-08-30T04:10:00+08:00")).expect("parse due date"), Some("2026-08-30T04:10:00+08:00"));
    }

    #[test]
    fn rejects_invalid_due_date_before_creating_reminder() {
        assert!(validate_due_date(Some("08/30/2026 04:10:00")).is_err());
    }
}
