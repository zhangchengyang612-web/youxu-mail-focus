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
            let script = r#"on run argv
set reminderTitle to item 1 of argv
set reminderNotes to item 2 of argv
set dueText to item 3 of argv
tell application "Reminders"
  tell default list
    set newReminder to make new reminder with properties {name:reminderTitle, body:reminderNotes}
    if dueText is not "" then set due date of newReminder to date dueText
    return id of newReminder
  end tell
end tell
end run"#;
            let due = draft.due_at.as_ref().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok()).map(|v| v.format("%m/%d/%Y %H:%M:%S").to_string()).unwrap_or_default();
            let out = Command::new("osascript").args(["-e", script, &draft.title, &draft.notes, &due]).output().map_err(|e| format!("无法启动提醒事项：{e}"))?;
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
