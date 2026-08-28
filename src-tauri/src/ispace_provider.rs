use crate::models::AssignmentDeadline;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};
use std::time::Duration;

pub fn fetch_assignments(calendar_url: &str) -> Result<Vec<AssignmentDeadline>, String> {
    let parsed = reqwest::Url::parse(calendar_url).map_err(|_| "iSpace 日历 URL 格式无效".to_string())?;
    let trusted_host = parsed.host_str().is_some_and(|host| host == "ispace.bnbu.edu.cn" || host.ends_with(".bnbu.edu.cn"));
    if parsed.scheme() != "https" || !trusted_host {
        return Err("请使用 https://ispace.bnbu.edu.cn 生成的日历 URL".into());
    }
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build().map_err(|_| "无法初始化 iSpace 同步".to_string())?
        .get(parsed).send().map_err(|error| {
            if error.is_timeout() {
                "直接连接 iSpace 超时，请检查网络后重试".to_string()
            } else if error.is_connect() {
                "无法直接连接 iSpace；当前网络可能无法访问学校服务器".to_string()
            } else {
                "无法通过 HTTPS 读取 iSpace 日历，请检查网络后重试".to_string()
            }
        })?;
    if !response.status().is_success() {
        return Err(format!("iSpace 日历拒绝访问（HTTP {}），请重新生成订阅 URL；若当前网络被学校限制，请联系 ITSC", response.status().as_u16()));
    }
    let bytes = response.bytes().map_err(|_| "无法读取 iSpace 日历内容".to_string())?;
    if bytes.len() > 5 * 1024 * 1024 { return Err("iSpace 日历数据异常过大".into()); }
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| "iSpace 返回的日历编码无效".to_string())?;
    if !text.contains("BEGIN:VCALENDAR") { return Err("iSpace 返回的不是日历数据，请检查订阅 URL 是否有效".into()); }
    Ok(parse_assignments(&text, Utc::now()))
}

pub fn parse_assignments(ics: &str, now: DateTime<Utc>) -> Vec<AssignmentDeadline> {
    let lines = unfold_lines(ics);
    let mut events = Vec::new();
    let mut current: Vec<(String, String)> = Vec::new();
    let mut inside = false;
    for line in lines {
        if line == "BEGIN:VEVENT" { inside = true; current.clear(); continue; }
        if line == "END:VEVENT" {
            if inside {
                if let Some(item) = event_to_assignment(&current, now) { events.push(item); }
            }
            inside = false;
            continue;
        }
        if inside {
            if let Some((key, value)) = line.split_once(':') {
                current.push((key.split(';').next().unwrap_or(key).to_uppercase(), unescape(value)));
            }
        }
    }
    events.sort_by(|a, b| a.due_at.cmp(&b.due_at));
    events
}

fn event_to_assignment(fields: &[(String, String)], now: DateTime<Utc>) -> Option<AssignmentDeadline> {
    let get = |name: &str| fields.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());
    let title = get("SUMMARY")?;
    let description = get("DESCRIPTION").unwrap_or_default();
    let url = get("URL");
    let haystack = format!("{} {} {}", title, description, url.as_deref().unwrap_or("")).to_lowercase();
    let assignment = ["/mod/assign/", "/mod/quiz/", "/mod/turnitintooltwo/", "/mod/workshop/", "assignment", "quiz", "turnitin", "作业", "测验", "提交", "报告", "project"]
        .iter().any(|marker| haystack.contains(marker));
    if !assignment { return None; }
    let due = parse_ical_date(&get("DTSTART")?)?;
    if due < now { return None; }
    let id = get("UID").unwrap_or_else(|| format!("{}:{}", title, due.timestamp()));
    let course = get("CATEGORIES").or_else(|| get("LOCATION")).unwrap_or_else(|| "iSpace 课程".into());
    let plain = if description.contains('<') { html2text::from_read(description.as_bytes(), 100) } else { description };
    Some(AssignmentDeadline { id, title, course, due_at: due.to_rfc3339(), description: plain.trim().to_string(), url, source: "iSpace".into() })
}

fn parse_ical_date(value: &str) -> Option<DateTime<Utc>> {
    if let Ok(date) = DateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ") { return Some(date.with_timezone(&Utc)); }
    let zone = FixedOffset::east_opt(8 * 3600)?;
    if let Ok(local) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S") {
        return zone.from_local_datetime(&local).single().map(|date| date.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(value, "%Y%m%d").ok()
        .and_then(|date| date.and_hms_opt(23, 59, 0))
        .and_then(|local| zone.from_local_datetime(&local).single())
        .map(|date| date.with_timezone(&Utc))
}

fn unfold_lines(input: &str) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();
    for line in input.replace("\r\n", "\n").split('\n') {
        if (line.starts_with(' ') || line.starts_with('\t')) && !output.is_empty() {
            output.last_mut().unwrap().push_str(line.trim_start());
        } else { output.push(line.to_string()); }
    }
    output
}

fn unescape(value: &str) -> String {
    value.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_future_assignments() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a1\r\nSUMMARY:Essay 1 Assignment\r\nCATEGORIES:Academic English\r\nDTSTART:20260905T180000\r\nURL:https://ispace.bnbu.edu.cn/mod/assign/view.php?id=1\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:e1\r\nSUMMARY:Campus event\r\nDTSTART:20260906T100000\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let now = DateTime::parse_from_rfc3339("2026-08-28T00:00:00Z").unwrap().with_timezone(&Utc);
        let items = parse_assignments(ics, now);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].course, "Academic English");
    }
}
