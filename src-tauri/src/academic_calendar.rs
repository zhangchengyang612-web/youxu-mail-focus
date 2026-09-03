use chrono::{FixedOffset, NaiveDate, TimeZone};

pub const CURRENT_SEMESTER: &str = "2026-27 第一学期";
pub const CURRENT_SOURCE_URL: &str = "https://ar.bnbu.edu.cn/attachment/file/Academic_Calendar_for_S1_AY202627.pdf";
pub const CURRENT_ENTRIES: &str = r#"2026-08-18 | 暑期学期结束（2025-26）
2026-08-23~2026-08-24 | 大一新生注册
2026-08-25~2026-08-31 | 新生迎新与英语强化课程
2026-09-01 | 第一学期开始 / 正式上课
2026-09-01~2026-09-14 | 选课增删期
2026-09-20 | 补星期五课程 / 全体教职员上班
2026-09-25~2026-09-27 | 中秋节假期
2026-10-01~2026-10-07 | 国庆节假期
2026-10-10 | 补星期一课程 / 全体教职员上班
2026-10-25~2026-10-31 | 阅读周
2026-12-11 | 第一学期最后上课日
2026-12-12 | 全国大学英语考试预留日
2026-12-13~2026-12-15 | 期末考试复习
2026-12-16~2026-12-27 | 第一学期期末考试
2026-12-28~2026-12-31 | 大一学生军训
2027-01-01 | 元旦假期
2027-01-02~2027-01-11 | 大一学生军训
2027-01-06 | 学部委员会会议
2027-01-08 | 校教务议会会议
2027-01-11 | 成绩发布 / 第一学期结束
2027-01-24~2027-01-31 | 春节假期（学校关闭）"#;

#[derive(Debug, Clone)]
pub struct AcademicCalendarEntry {
    pub title: String,
    pub starts_at: String,
}

pub fn validate_source_url(value: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(value.trim()).map_err(|_| "BNBU 校历来源 URL 格式无效".to_string())?;
    let trusted = parsed.scheme() == "https"
        && parsed.host_str().is_some_and(|host| host == "ar.bnbu.edu.cn");
    if !trusted { return Err("请使用 BNBU 教务处 ar.bnbu.edu.cn 的 HTTPS 校历链接".into()); }
    Ok(())
}

pub fn parse_entries(value: &str) -> Result<Vec<AcademicCalendarEntry>, String> {
    let zone = FixedOffset::east_opt(8 * 3600).ok_or_else(|| "无法初始化中国时区".to_string())?;
    let mut output = Vec::new();
    for (index, raw) in value.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        let (range, title) = line.split_once('|').ok_or_else(|| format!("第 {} 行格式错误，应为 日期 | 事件", index + 1))?;
        let title = title.trim();
        if title.is_empty() { return Err(format!("第 {} 行缺少事件名称", index + 1)); }
        let (start, end) = range.trim().split_once('~').unwrap_or((range.trim(), range.trim()));
        let mut date = NaiveDate::parse_from_str(start.trim(), "%Y-%m-%d").map_err(|_| format!("第 {} 行开始日期无效", index + 1))?;
        let end = NaiveDate::parse_from_str(end.trim(), "%Y-%m-%d").map_err(|_| format!("第 {} 行结束日期无效", index + 1))?;
        if end < date || (end - date).num_days() > 120 { return Err(format!("第 {} 行日期范围无效或超过 120 天", index + 1)); }
        loop {
            let starts_at = zone.from_local_datetime(&date.and_hms_opt(9, 0, 0).unwrap()).single().unwrap().to_rfc3339();
            output.push(AcademicCalendarEntry { title: title.to_string(), starts_at });
            if date == end { break; }
            date = date.succ_opt().ok_or_else(|| format!("第 {} 行日期超出范围", index + 1))?;
        }
    }
    if output.is_empty() { return Err("请至少填写一条校历事件".into()); }
    if output.len() > 500 { return Err("单个学期最多导入 500 个校历日期".into()); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_academic_calendar_date_ranges() {
        let events = parse_entries("2026-09-01~2026-09-03 | 选课增删期\n2026-12-11 | 最后上课日").unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].title, "选课增删期");
        assert!(events[3].starts_at.starts_with("2026-12-11T09:00:00"));
    }
}
