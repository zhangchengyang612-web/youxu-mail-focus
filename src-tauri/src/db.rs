use crate::models::{AssignmentDeadline, ClassificationRule, MailMessage, MailSettings, ParsedMail};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
      CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS mails (
        id TEXT PRIMARY KEY, uid INTEGER NOT NULL, folder TEXT NOT NULL DEFAULT 'INBOX', sender_name TEXT NOT NULL,
        sender_email TEXT NOT NULL, recipients TEXT NOT NULL DEFAULT '[]', subject TEXT NOT NULL, received_at TEXT NOT NULL,
        body_text TEXT NOT NULL, category TEXT NOT NULL, classification_reason TEXT NOT NULL, is_read INTEGER NOT NULL DEFAULT 0,
        reminder_status TEXT NOT NULL DEFAULT 'none', UNIQUE(folder, uid));
      CREATE TABLE IF NOT EXISTS rules (
        id TEXT PRIMARY KEY, category TEXT NOT NULL, field TEXT NOT NULL, operator TEXT NOT NULL,
        value TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1);
      CREATE TABLE IF NOT EXISTS reminder_links (
        source_mail_id TEXT PRIMARY KEY, platform TEXT NOT NULL, external_id TEXT NOT NULL,
        created_at TEXT NOT NULL, status TEXT NOT NULL, FOREIGN KEY(source_mail_id) REFERENCES mails(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS sync_state (
        folder TEXT PRIMARY KEY, last_uid INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS assignments (
        id TEXT PRIMARY KEY, title TEXT NOT NULL, course TEXT NOT NULL, due_at TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '', url TEXT, source TEXT NOT NULL DEFAULT 'iSpace');
      CREATE INDEX IF NOT EXISTS idx_mails_received ON mails(received_at DESC);")
      .map_err(|e| e.to_string())?;
    connection.execute_batch("UPDATE mails SET category = CASE category
      WHEN '会议' THEN '学业' WHEN '审批' THEN '校园事务' WHEN '财务' THEN '个人'
      WHEN '通知' THEN '校园事务' WHEN '其他' THEN '外部' ELSE category END;
      DELETE FROM rules WHERE category IN ('会议','审批','财务','通知','其他');")
      .map_err(|e| e.to_string())?;
    let migrated: Option<String> = connection.query_row(
        "SELECT value FROM settings WHERE key='future_only_0_2_2'",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if migrated.is_none() {
        connection.execute_batch("DELETE FROM reminder_links;
          DELETE FROM mails;
          DELETE FROM sync_state;
          INSERT INTO settings(key,value) VALUES('future_only_0_2_2','1');")
          .map_err(|e| e.to_string())?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")
          .map_err(|e| e.to_string())?;
    }
    Ok(connection)
}

pub fn save_settings(db: &Connection, settings: &MailSettings) -> Result<(), String> {
    let value = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    db.execute("INSERT INTO settings(key,value) VALUES('mail',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_settings(db: &Connection) -> Result<MailSettings, String> {
    let value: Option<String> = db.query_row("SELECT value FROM settings WHERE key='mail'", [], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    value.ok_or_else(|| "请先在设置中连接 BNBU 学生邮箱".to_string()).and_then(|v| serde_json::from_str(&v).map_err(|e| e.to_string()))
}

pub fn load_sync_cursor(db: &Connection) -> Result<Option<u32>, String> {
    db.query_row("SELECT last_uid FROM sync_state WHERE folder='INBOX'", [], |row| row.get(0))
        .optional().map_err(|e| e.to_string())
}

pub fn save_sync_cursor(db: &Connection, uid: u32) -> Result<(), String> {
    db.execute("INSERT INTO sync_state(folder,last_uid) VALUES('INBOX',?1)
      ON CONFLICT(folder) DO UPDATE SET last_uid=excluded.last_uid", [uid])
      .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn reset_sync_from_now(db: &Connection) -> Result<(), String> {
    db.execute_batch("DELETE FROM reminder_links; DELETE FROM mails; DELETE FROM sync_state;")
        .map_err(|e| e.to_string())
}

pub fn replace_assignments(db: &Connection, assignments: &[AssignmentDeadline]) -> Result<(), String> {
    db.execute("DELETE FROM assignments WHERE source='iSpace'", []).map_err(|e| e.to_string())?;
    for item in assignments {
        db.execute("INSERT INTO assignments(id,title,course,due_at,description,url,source)
          VALUES(?1,?2,?3,?4,?5,?6,?7)", params![item.id,item.title,item.course,item.due_at,item.description,item.url,item.source])
          .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn list_assignments(db: &Connection) -> Result<Vec<AssignmentDeadline>, String> {
    let mut statement = db.prepare("SELECT id,title,course,due_at,description,url,source FROM assignments ORDER BY due_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok(AssignmentDeadline {
        id: row.get(0)?, title: row.get(1)?, course: row.get(2)?, due_at: row.get(3)?,
        description: row.get(4)?, url: row.get(5)?, source: row.get(6)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

pub fn upsert_mail(db: &Connection, mail: &ParsedMail, category: &str, reason: &str) -> Result<(), String> {
    let id = format!("INBOX:{}", mail.uid);
    db.execute("INSERT INTO mails(id,uid,folder,sender_name,sender_email,recipients,subject,received_at,body_text,category,classification_reason,is_read)
      VALUES(?1,?2,'INBOX',?3,?4,?5,?6,?7,?8,?9,?10,?11)
      ON CONFLICT(folder,uid) DO UPDATE SET sender_name=excluded.sender_name,sender_email=excluded.sender_email,recipients=excluded.recipients,subject=excluded.subject,received_at=excluded.received_at,body_text=excluded.body_text,is_read=excluded.is_read",
      params![id, mail.uid, mail.sender_name, mail.sender_email, serde_json::to_string(&mail.recipients).unwrap_or_default(), mail.subject, mail.received_at, mail.body_text, category, reason, mail.is_read as i32]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_mails(db: &Connection) -> Result<Vec<MailMessage>, String> {
    let mut statement = db.prepare("SELECT id,uid,folder,sender_name,sender_email,recipients,subject,received_at,body_text,category,classification_reason,is_read,reminder_status FROM mails ORDER BY received_at DESC").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |r| Ok(MailMessage { id:r.get(0)?, uid:r.get(1)?, folder:r.get(2)?, sender_name:r.get(3)?, sender_email:r.get(4)?, recipients:serde_json::from_str(&r.get::<_,String>(5)?).unwrap_or_default(), subject:r.get(6)?, received_at:r.get(7)?, body_text:r.get(8)?, category:r.get(9)?, classification_reason:r.get(10)?, is_read:r.get::<_,i32>(11)? != 0, reminder_status:r.get(12)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

pub fn update_category(db: &Connection, id: &str, category: &str) -> Result<(), String> {
    db.execute("UPDATE mails SET category=?1,classification_reason='手动分类' WHERE id=?2", params![category,id]).map_err(|e| e.to_string())?; Ok(())
}

pub fn list_rules(db: &Connection) -> Result<Vec<ClassificationRule>, String> {
    let mut statement = db.prepare("SELECT id,category,field,operator,value,priority,enabled FROM rules ORDER BY priority DESC").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |r| Ok(ClassificationRule { id:r.get(0)?,category:r.get(1)?,field:r.get(2)?,operator:r.get(3)?,value:r.get(4)?,priority:r.get(5)?,enabled:r.get::<_,i32>(6)? != 0 })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

pub fn save_rule(db: &Connection, rule: &ClassificationRule) -> Result<(), String> {
    db.execute("INSERT INTO rules(id,category,field,operator,value,priority,enabled) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET category=excluded.category,field=excluded.field,operator=excluded.operator,value=excluded.value,priority=excluded.priority,enabled=excluded.enabled", params![rule.id,rule.category,rule.field,rule.operator,rule.value,rule.priority,rule.enabled as i32]).map_err(|e| e.to_string())?; Ok(())
}

pub fn delete_rule(db: &Connection, id: &str) -> Result<(), String> { db.execute("DELETE FROM rules WHERE id=?1", [id]).map_err(|e| e.to_string())?; Ok(()) }

pub fn save_reminder_link(db: &Connection, mail_id: &str, platform: &str, external_id: &str) -> Result<(), String> {
    let exists: Option<String> = db.query_row("SELECT external_id FROM reminder_links WHERE source_mail_id=?1 AND status='created'", [mail_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if exists.is_some() { return Err("这封邮件已经创建过提醒".into()); }
    db.execute("INSERT INTO reminder_links(source_mail_id,platform,external_id,created_at,status) VALUES(?1,?2,?3,datetime('now'),'created')", params![mail_id,platform,external_id]).map_err(|e| e.to_string())?;
    db.execute("UPDATE mails SET reminder_status='created' WHERE id=?1", [mail_id]).map_err(|e| e.to_string())?; Ok(())
}

pub fn clear(db: &Connection) -> Result<(), String> { db.execute_batch("DELETE FROM reminder_links; DELETE FROM mails; DELETE FROM rules; DELETE FROM assignments; DELETE FROM settings;").map_err(|e| e.to_string()) }
