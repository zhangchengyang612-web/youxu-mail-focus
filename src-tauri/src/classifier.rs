use crate::models::{ClassificationRule, ParsedMail};

const GROUPS: &[(&str, &[&str])] = &[
    ("待办", &["请处理", "请完成", "待办", "action required", "deadline", "截止", "due date", "务必"]),
    ("学业", &["课程", "作业", "考试", "成绩", "课堂", "选课", "论文", "导师", "assignment", "course", "exam", "quiz", "lecture"]),
    ("校园事务", &["教务", "校园", "宿舍", "图书馆", "注册", "缴费", "学生事务", "itsc", "校园卡", "系统通知"]),
    ("社团活动", &["社团", "活动", "招募", "志愿者", "比赛", "报名", "讲座", "工作坊", "club", "event", "workshop"]),
    ("个人", &["个人", "预约", "账单", "快递", "账户", "生日", "appointment", "personal"]),
];

pub fn classify(mail: &ParsedMail, rules: &[ClassificationRule]) -> (String, String) {
    let mut sorted = rules.to_vec(); sorted.sort_by_key(|rule| -rule.priority);
    for rule in sorted.iter().filter(|r| r.enabled) {
        let source = match rule.field.as_str() { "sender" => mail.sender_email.as_str(), "domain" => mail.sender_email.split('@').nth(1).unwrap_or(""), "subject" => mail.subject.as_str(), "body" => mail.body_text.as_str(), _ => continue };
        let matched = match rule.operator.as_str() { "equals" => source.eq_ignore_ascii_case(&rule.value), "contains" => source.to_lowercase().contains(&rule.value.to_lowercase()), "regex" => regex::RegexBuilder::new(&rule.value).case_insensitive(true).build().map(|r| r.is_match(source)).unwrap_or(false), _ => false };
        if matched { return (rule.category.clone(), format!("规则：{} {} {}", rule.field, rule.operator, rule.value)); }
    }
    let text = format!("{}\n{}", mail.subject, mail.body_text).to_lowercase();
    for (category, words) in GROUPS {
        if let Some(word) = words.iter().find(|word| text.contains(&word.to_lowercase())) {
            return (category.to_string(), format!("关键词：{word}"));
        }
    }
    ("外部".into(), "未匹配校内或个人规则".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_action_mail() {
        let mail = ParsedMail { uid: 1, sender_name: "A".into(), sender_email: "a@x.com".into(), recipients: vec![], subject: "请处理合同".into(), received_at: "".into(), body_text: "".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "待办");
    }
}
