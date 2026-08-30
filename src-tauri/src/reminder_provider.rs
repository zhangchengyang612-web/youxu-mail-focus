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
            let due = required_due_date(draft.due_at.as_deref())?;
            let task_id = format!("YouXu-{}", Uuid::new_v4());
            let title = draft.title.chars().take(200).collect::<String>();
            let notes = draft.notes.chars().take(1200).collect::<String>();
            let script = r#"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$at = [DateTimeOffset]::Parse($env:YOUXU_DUE, [Globalization.CultureInfo]::InvariantCulture).LocalDateTime
if ($at -le (Get-Date)) { throw '提醒时间必须晚于当前时间' }
function Quote-PowerShell([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
$message = $env:YOUXU_TITLE
if ($env:YOUXU_NOTES) { $message += "`n`n" + $env:YOUXU_NOTES }
$child = "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show($(Quote-PowerShell $message), '邮序提醒') | Out-Null"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($child))
$action = New-ScheduledTaskAction -Execute "$PSHOME\powershell.exe" -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encoded"
$trigger = New-ScheduledTaskTrigger -Once -At $at
$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $env:YOUXU_TASK -Action $action -Trigger $trigger -Principal $principal -Description '邮序本机提醒' -Force | Out-Null
"#;
            let out = Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
                .env("YOUXU_DUE", due)
                .env("YOUXU_TASK", &task_id)
                .env("YOUXU_TITLE", title)
                .env("YOUXU_NOTES", notes)
                .output()
                .map_err(|e| format!("无法启动 Windows 任务计划程序：{e}"))?;
            if !out.status.success() {
                return Err(format!("Windows 本机提醒创建失败：{}", String::from_utf8_lossy(&out.stderr).trim()));
            }
            return Ok(("windows-task-scheduler".into(), task_id));
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

#[cfg(any(target_os = "windows", test))]
fn required_due_date(value: Option<&str>) -> Result<&str, String> {
    validate_due_date(value)?.ok_or_else(|| "Windows 本机提醒必须设置日期和时间".to_string())
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

    #[test]
    fn windows_local_reminder_requires_a_due_date() {
        assert!(required_due_date(None).is_err());
        assert_eq!(required_due_date(Some("2026-08-31T18:00:00+08:00")).unwrap(), "2026-08-31T18:00:00+08:00");
    }
}
