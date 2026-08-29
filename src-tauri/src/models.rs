use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSettings { pub host: String, pub port: u16, pub email: String, pub initial_days: u32, pub sync_minutes: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailMessage {
    pub id: String, pub uid: u32, pub folder: String, pub sender_name: String,
    pub sender_email: String, pub recipients: Vec<String>, pub subject: String,
    pub received_at: String, pub body_text: String, pub category: String,
    pub classification_reason: String, pub is_read: bool, pub reminder_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDraft {
    pub title: String, pub notes: String, pub due_at: Option<String>, pub priority: String,
    pub source_mail_id: String, pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationRule {
    pub id: String, pub category: String, pub field: String, pub operator: String,
    pub value: String, pub priority: i32, pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedMail { pub uid: u32, pub sender_name: String, pub sender_email: String, pub recipients: Vec<String>, pub subject: String, pub received_at: String, pub body_text: String, pub is_read: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentDeadline {
    pub id: String,
    pub title: String,
    pub course: String,
    pub due_at: String,
    pub description: String,
    pub url: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub starts_at: Option<String>,
    pub priority: String,
    pub kind: String,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalReminderInput {
    pub id: Option<String>,
    pub title: String,
    pub notes: String,
    pub starts_at: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemReminderInput {
    pub source_id: String,
    pub title: String,
    pub notes: String,
    pub due_at: String,
    pub priority: String,
}
