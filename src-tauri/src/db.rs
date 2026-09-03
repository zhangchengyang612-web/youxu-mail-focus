use crate::academic_calendar::AcademicCalendarEntry;
use crate::models::{AssignmentDeadline, CalendarEvent, ClassificationRule, MailMessage, MailSettings, ParsedMail, PersonalReminderInput, ReminderDraft};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
      CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS mails (
        id TEXT PRIMARY KEY, uid INTEGER NOT NULL, folder TEXT NOT NULL DEFAULT 'INBOX', sender_name TEXT NOT NULL,
        sender_email TEXT NOT NULL, recipients TEXT NOT NULL DEFAULT '[]', subject TEXT NOT NULL, received_at TEXT NOT NULL,
        body_text TEXT NOT NULL, category TEXT NOT NULL, classification_reason TEXT NOT NULL, is_todo INTEGER NOT NULL DEFAULT 0, is_read INTEGER NOT NULL DEFAULT 0,
        reminder_status TEXT NOT NULL DEFAULT 'none', UNIQUE(folder, uid));
      CREATE TABLE IF NOT EXISTS rules (
        id TEXT PRIMARY KEY, category TEXT NOT NULL, field TEXT NOT NULL, operator TEXT NOT NULL,
        value TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1);
      CREATE TABLE IF NOT EXISTS reminder_links (
        source_mail_id TEXT PRIMARY KEY, platform TEXT NOT NULL, external_id TEXT NOT NULL,
        created_at TEXT NOT NULL, status TEXT NOT NULL, FOREIGN KEY(source_mail_id) REFERENCES mails(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS system_reminder_links (
        source_id TEXT PRIMARY KEY, platform TEXT NOT NULL, external_id TEXT NOT NULL,
        created_at TEXT NOT NULL, status TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS sync_state (
        folder TEXT PRIMARY KEY, last_uid INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS assignments (
        id TEXT PRIMARY KEY, title TEXT NOT NULL, course TEXT NOT NULL, due_at TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '', url TEXT, source TEXT NOT NULL DEFAULT 'iSpace');
      CREATE TABLE IF NOT EXISTS calendar_events (
        id TEXT PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL DEFAULT '', starts_at TEXT,
        priority TEXT NOT NULL DEFAULT 'normal', kind TEXT NOT NULL,
        source_id TEXT, source_url TEXT, read_only INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
      CREATE INDEX IF NOT EXISTS idx_mails_received ON mails(received_at DESC);")
      .map_err(|e| e.to_string())?;
    let has_todo = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('mails') WHERE name='is_todo')",
        [],
        |row| row.get::<_, bool>(0),
    ).map_err(|e| e.to_string())?;
    if !has_todo {
        connection.execute("ALTER TABLE mails ADD COLUMN is_todo INTEGER NOT NULL DEFAULT 0", []).map_err(|e| e.to_string())?;
    }
    connection.execute_batch("UPDATE mails SET category = CASE category
      WHEN '会议' THEN '学业' WHEN '审批' THEN '校园事务' WHEN '财务' THEN '个人'
      WHEN '通知' THEN '校园事务' WHEN '其他' THEN '外部' ELSE category END;
      DELETE FROM rules WHERE category IN ('会议','审批','财务','通知','其他');
      UPDATE mails SET category='外部', is_todo=CASE WHEN classification_reason='手动分类' THEN 1 ELSE 0 END,
        classification_reason=CASE WHEN classification_reason='手动分类' THEN '手动分类' ELSE '待重新分类' END WHERE category='待办';
      UPDATE mails SET is_todo=0, classification_reason='待重新分类' WHERE classification_reason='原待办分类已迁移为手动标签';
      DELETE FROM rules WHERE category='待办';")
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

pub fn infer_sender_name(db: &Connection, email: &str) -> Result<Option<String>, String> {
    let local_part = email.split('@').next().unwrap_or_default();
    db.query_row(
        "SELECT sender_name FROM mails WHERE lower(sender_email)=lower(?1) AND trim(sender_name)<>'' AND lower(trim(sender_name)) NOT IN (lower(?1),lower(?2)) ORDER BY received_at DESC LIMIT 1",
        params![email, local_part],
        |row| row.get(0),
    ).optional().map_err(|error| error.to_string())
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
    let mut statement = db.prepare("SELECT id,uid,folder,sender_name,sender_email,recipients,subject,received_at,body_text,category,classification_reason,is_todo,is_read,reminder_status FROM mails ORDER BY received_at DESC").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |r| Ok(MailMessage { id:r.get(0)?, uid:r.get(1)?, folder:r.get(2)?, sender_name:r.get(3)?, sender_email:r.get(4)?, recipients:serde_json::from_str(&r.get::<_,String>(5)?).unwrap_or_default(), subject:r.get(6)?, received_at:r.get(7)?, body_text:crate::mail_provider::normalize_body_text(r.get(8)?), category:r.get(9)?, classification_reason:r.get(10)?, is_todo:r.get::<_,i32>(11)? != 0, is_read:r.get::<_,i32>(12)? != 0, reminder_status:r.get(13)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

pub fn update_category(db: &Connection, id: &str, category: &str) -> Result<(), String> {
    db.execute("UPDATE mails SET category=?1,classification_reason='手动分类' WHERE id=?2", params![category,id]).map_err(|e| e.to_string())?; Ok(())
}

pub fn update_todo(db: &Connection, id: &str, is_todo: bool) -> Result<(), String> {
    db.execute("UPDATE mails SET is_todo=?1 WHERE id=?2", params![is_todo as i32,id]).map_err(|e| e.to_string())?; Ok(())
}

pub fn update_classification(db: &Connection, id: &str, category: &str, reason: &str) -> Result<(), String> {
    db.execute("UPDATE mails SET category=?1,classification_reason=?2 WHERE id=?3", params![category,reason,id]).map_err(|e| e.to_string())?; Ok(())
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

pub fn reminder_exists(db: &Connection, mail_id: &str) -> Result<bool, String> {
    let exists: Option<String> = db.query_row("SELECT external_id FROM reminder_links WHERE source_mail_id=?1 AND status='created'", [mail_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    Ok(exists.is_some())
}

pub fn system_reminder_exists(db: &Connection, source_id: &str) -> Result<bool, String> {
    let exists: Option<String> = db.query_row("SELECT external_id FROM system_reminder_links WHERE source_id=?1 AND status='created'", [source_id], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
    Ok(exists.is_some())
}

pub fn save_system_reminder_link(db: &Connection, source_id: &str, platform: &str, external_id: &str) -> Result<(), String> {
    db.execute("INSERT INTO system_reminder_links(source_id,platform,external_id,created_at,status) VALUES(?1,?2,?3,datetime('now'),'created')", params![source_id,platform,external_id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_system_reminder_sources(db: &Connection) -> Result<Vec<String>, String> {
    let mut statement = db.prepare("SELECT source_id FROM system_reminder_links WHERE status='created' ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

pub fn save_created_reminder(db: &mut Connection, draft: &ReminderDraft, platform: &str, external_id: &str) -> Result<(), String> {
    let transaction = db.transaction().map_err(|e| e.to_string())?;
    transaction.execute("INSERT INTO reminder_links(source_mail_id,platform,external_id,created_at,status) VALUES(?1,?2,?3,datetime('now'),'created')", params![draft.source_mail_id,platform,external_id]).map_err(|e| e.to_string())?;
    transaction.execute("UPDATE mails SET reminder_status='created' WHERE id=?1", [&draft.source_mail_id]).map_err(|e| e.to_string())?;
    transaction.execute("INSERT INTO calendar_events(id,title,notes,starts_at,priority,kind,source_id,source_url,read_only,updated_at)
      VALUES(?1,?2,?3,?4,?5,'mail',?6,?7,1,datetime('now'))
      ON CONFLICT(id) DO UPDATE SET title=excluded.title,notes=excluded.notes,starts_at=excluded.starts_at,
      priority=excluded.priority,source_url=excluded.source_url,updated_at=datetime('now')",
      params![format!("mail:{}", draft.source_mail_id), draft.title, draft.notes, draft.due_at, draft.priority, draft.source_mail_id, draft.source_url]).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

pub fn list_calendar_events(db: &Connection) -> Result<Vec<CalendarEvent>, String> {
    let mut statement = db.prepare("SELECT id,title,notes,starts_at,priority,kind,source_id,source_url,read_only FROM calendar_events ORDER BY COALESCE(starts_at,'9999') ASC")
        .map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok(CalendarEvent {
        id: row.get(0)?, title: row.get(1)?, notes: row.get(2)?, starts_at: row.get(3)?,
        priority: row.get(4)?, kind: row.get(5)?, source_id: row.get(6)?, source_url: row.get(7)?,
        read_only: row.get::<_, i32>(8)? != 0,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

pub fn academic_calendar_is_empty(db: &Connection) -> Result<bool, String> {
    let count: i64 = db.query_row("SELECT COUNT(*) FROM calendar_events WHERE kind='academic'", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    Ok(count == 0)
}

pub fn replace_academic_calendar(db: &mut Connection, semester: &str, source_url: &str, entries: &[AcademicCalendarEntry]) -> Result<(), String> {
    let transaction = db.transaction().map_err(|e| e.to_string())?;
    transaction.execute("DELETE FROM calendar_events WHERE kind='academic' AND source_id=?1", [semester]).map_err(|e| e.to_string())?;
    for (index, entry) in entries.iter().enumerate() {
        transaction.execute("INSERT INTO calendar_events(id,title,notes,starts_at,priority,kind,source_id,source_url,read_only,updated_at)
          VALUES(?1,?2,?3,?4,'normal','academic',?5,?6,1,datetime('now'))",
          params![format!("academic:{semester}:{}:{index}", entry.starts_at), entry.title, semester, entry.starts_at, semester, source_url]).map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())
}

pub fn save_personal_event(db: &Connection, input: &PersonalReminderInput) -> Result<CalendarEvent, String> {
    let id = input.id.clone().unwrap_or_else(|| format!("personal:{}", uuid::Uuid::new_v4()));
    db.execute("INSERT INTO calendar_events(id,title,notes,starts_at,priority,kind,read_only,updated_at)
      VALUES(?1,?2,?3,?4,?5,'personal',0,datetime('now'))
      ON CONFLICT(id) DO UPDATE SET title=excluded.title,notes=excluded.notes,starts_at=excluded.starts_at,
      priority=excluded.priority,updated_at=datetime('now') WHERE calendar_events.kind='personal'",
      params![id,input.title.trim(),input.notes.trim(),input.starts_at,input.priority]).map_err(|e| e.to_string())?;
    db.query_row("SELECT id,title,notes,starts_at,priority,kind,source_id,source_url,read_only FROM calendar_events WHERE id=?1 AND kind='personal'", [&id], |row| Ok(CalendarEvent {
        id: row.get(0)?, title: row.get(1)?, notes: row.get(2)?, starts_at: row.get(3)?,
        priority: row.get(4)?, kind: row.get(5)?, source_id: row.get(6)?, source_url: row.get(7)?, read_only: row.get::<_,i32>(8)? != 0,
    })).map_err(|e| e.to_string())
}

pub fn delete_personal_event(db: &Connection, id: &str) -> Result<(), String> {
    let changed = db.execute("DELETE FROM calendar_events WHERE id=?1 AND kind='personal'", [id]).map_err(|e| e.to_string())?;
    if changed == 0 { return Err("只能删除个人提醒".into()); }
    Ok(())
}

pub fn clear(db: &Connection) -> Result<(), String> { db.execute_batch("DELETE FROM reminder_links; DELETE FROM system_reminder_links; DELETE FROM mails; DELETE FROM rules; DELETE FROM assignments; DELETE FROM calendar_events; DELETE FROM settings;").map_err(|e| e.to_string()) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_updates_the_independent_todo_tag() {
        let path = std::env::temp_dir().join(format!("mail-focus-todo-{}.sqlite", uuid::Uuid::new_v4()));
        let legacy = Connection::open(&path).expect("open legacy database");
        legacy.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO settings(key,value) VALUES('future_only_0_2_2','1');
            CREATE TABLE mails (
              id TEXT PRIMARY KEY, uid INTEGER NOT NULL, folder TEXT NOT NULL DEFAULT 'INBOX', sender_name TEXT NOT NULL,
              sender_email TEXT NOT NULL, recipients TEXT NOT NULL DEFAULT '[]', subject TEXT NOT NULL, received_at TEXT NOT NULL,
              body_text TEXT NOT NULL, category TEXT NOT NULL, classification_reason TEXT NOT NULL, is_read INTEGER NOT NULL DEFAULT 0,
              reminder_status TEXT NOT NULL DEFAULT 'none', UNIQUE(folder, uid));
            INSERT INTO mails(id,uid,sender_name,sender_email,subject,received_at,body_text,category,classification_reason)
              VALUES('todo-test',1,'A','a@example.com','请处理','2026-09-03','正文','待办','旧分类');
            INSERT INTO mails(id,uid,sender_name,sender_email,subject,received_at,body_text,category,classification_reason)
              VALUES('manual-todo',2,'B','b@example.com','人工待办','2026-09-02','正文','待办','手动分类');").expect("create legacy database");
        drop(legacy);

        let db = open(&path).expect("reopen migrated database");
        let mails = list_mails(&db).expect("list migrated mail");
        let mail = mails.iter().find(|mail| mail.id == "todo-test").unwrap();
        let manual = mails.iter().find(|mail| mail.id == "manual-todo").unwrap();
        assert_eq!(mail.category, "外部");
        assert!(!mail.is_todo);
        assert_eq!(manual.category, "外部");
        assert!(manual.is_todo);
        update_todo(&db, &mail.id, true).expect("add todo tag");
        let updated = list_mails(&db).expect("list updated mail").into_iter().find(|mail| mail.id == "todo-test").unwrap();
        assert_eq!(updated.category, "外部");
        assert!(updated.is_todo);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn infers_the_latest_formal_sender_name_for_the_connected_account() {
        let path = std::env::temp_dir().join(format!("mail-focus-identity-{}.sqlite", uuid::Uuid::new_v4()));
        let db = open(&path).expect("open test database");
        db.execute(
            "INSERT INTO mails(id,uid,sender_name,sender_email,subject,received_at,body_text,category,classification_reason) VALUES(?1,?2,?3,?4,'测试','2026-08-31T12:00:00+08:00','正文','个人','测试')",
            params!["identity-test", 1, "Student Name", "student@example.edu"],
        ).expect("insert self mail");
        db.execute(
            "INSERT INTO mails(id,uid,sender_name,sender_email,subject,received_at,body_text,category,classification_reason) VALUES(?1,?2,?3,?4,'App 测试','2026-08-31T21:55:00+08:00','正文','个人','测试')",
            params!["identity-local-part", 2, "student", "student@example.edu"],
        ).expect("insert newer app mail");
        assert_eq!(infer_sender_name(&db, "student@example.edu").unwrap(), Some("Student Name".into()));
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn saves_updates_and_deletes_personal_calendar_events() {
        let path = std::env::temp_dir().join(format!("mail-focus-calendar-{}.sqlite", uuid::Uuid::new_v4()));
        let db = open(&path).expect("open test database");
        let input = PersonalReminderInput { id: None, title: "复习课程".into(), notes: "第 3 章".into(), starts_at: "2026-09-02T10:00:00+08:00".into(), priority: "high".into() };
        let created = save_personal_event(&db, &input).expect("save personal event");
        assert_eq!(created.kind, "personal");
        assert!(!created.read_only);
        assert_eq!(list_calendar_events(&db).expect("list events").len(), 1);

        let updated = PersonalReminderInput { id: Some(created.id.clone()), title: "复习课程（更新）".into(), ..input };
        assert_eq!(save_personal_event(&db, &updated).expect("update event").title, "复习课程（更新）");
        delete_personal_event(&db, &created.id).expect("delete personal event");
        assert!(list_calendar_events(&db).expect("list after delete").is_empty());
        assert!(!system_reminder_exists(&db, "personal:test").expect("check system reminder"));
        save_system_reminder_link(&db, "personal:test", "apple-reminders", "external-1").expect("save system reminder link");
        assert!(system_reminder_exists(&db, "personal:test").expect("check saved system reminder"));
        assert_eq!(list_system_reminder_sources(&db).expect("list sources"), vec!["personal:test"]);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn replaces_only_the_selected_academic_semester() {
        let path = std::env::temp_dir().join(format!("mail-focus-academic-{}.sqlite", uuid::Uuid::new_v4()));
        let mut db = open(&path).expect("open test database");
        let first = vec![AcademicCalendarEntry { title: "开学".into(), starts_at: "2026-09-01T09:00:00+08:00".into() }];
        replace_academic_calendar(&mut db, "2026-27 第一学期", "https://ar.bnbu.edu.cn/calendar.pdf", &first).expect("import term");
        assert!(!academic_calendar_is_empty(&db).unwrap());
        let updated = vec![AcademicCalendarEntry { title: "更新后的开学日".into(), starts_at: "2026-09-02T09:00:00+08:00".into() }];
        replace_academic_calendar(&mut db, "2026-27 第一学期", "https://ar.bnbu.edu.cn/calendar.pdf", &updated).expect("replace term");
        let events = list_calendar_events(&db).unwrap().into_iter().filter(|item| item.kind == "academic").collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "更新后的开学日");
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
