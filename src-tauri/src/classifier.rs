use crate::models::{ClassificationRule, ParsedMail};

const GROUPS: &[(&str, &[&str])] = &[
    ("实习", &["实习", "实习生", "校招", "招聘", "简历", "internship", "intern position", "recruitment", "student assistant", "career opportunity", "job opening", "resume"]),
    ("学业", &["课程", "作业", "考试", "成绩", "课堂", "选课", "论文", "导师", "assignment", "course", "exam", "quiz", "lecture"]),
    ("校园事务", &["教务", "校园", "宿舍", "图书馆", "注册", "缴费", "学生事务", "itsc", "校园卡", "系统通知", "学生职业发展中心", "职业发展规划", "cdc宣讲会", "career development center", "career development centre", "一对一咨询项目", "创新中心", "创客中心", "入孵项目", "校赛", "项目打磨交流会"]),
    ("社团活动", &["社团", "活动", "招募", "志愿者", "比赛", "报名", "讲座", "工作坊", "club", "event", "workshop"]),
    ("个人", &["个人", "预约", "账单", "快递", "账户", "生日", "appointment", "personal"]),
];
const CAMPUS_EVENT_PHRASES: &[&str] = &["系列讲座", "dls lecture", "guest lecture", "datathon"];
const PERSONAL_ITEM_PHRASES: &[&str] = &["物品领取", "物品认领", "认领物品", "宿舍安全检查", "宿舍安全专项检查", "student hostel safety inspection"];
const CAMPUS_SENDERS: &[&str] = &["career@uic.edu.cn", "career@bnbu.edu.cn", "ic@bnbu.edu.cn"];

pub fn classify(mail: &ParsedMail, rules: &[ClassificationRule]) -> (String, String) {
    let mut sorted = rules.to_vec(); sorted.sort_by_key(|rule| -rule.priority);
    for rule in sorted.iter().filter(|r| r.enabled) {
        let source = match rule.field.as_str() { "sender" => mail.sender_email.as_str(), "domain" => mail.sender_email.split('@').nth(1).unwrap_or(""), "subject" => mail.subject.as_str(), "body" => mail.body_text.as_str(), _ => continue };
        let matched = match rule.operator.as_str() { "equals" => source.eq_ignore_ascii_case(&rule.value), "contains" => source.to_lowercase().contains(&rule.value.to_lowercase()), "regex" => regex::RegexBuilder::new(&rule.value).case_insensitive(true).build().map(|r| r.is_match(source)).unwrap_or(false), _ => false };
        if matched { return (rule.category.clone(), format!("规则：{} {} {}", rule.field, rule.operator, rule.value)); }
    }
    let text = format!("{}\n{}", mail.subject, mail.body_text).to_lowercase();
    if let Some(word) = GROUPS[0].1.iter().find(|word| text.contains(*word)) {
        return ("实习".into(), format!("关键词：{word}"));
    }
    if let Some(word) = PERSONAL_ITEM_PHRASES.iter().find(|word| text.contains(*word)) {
        return ("个人".into(), format!("关键词：{word}"));
    }
    if let Some(word) = CAMPUS_EVENT_PHRASES.iter().find(|word| text.contains(*word)) {
        return ("校园事务".into(), format!("关键词：{word}"));
    }
    if CAMPUS_SENDERS.iter().any(|sender| mail.sender_email.eq_ignore_ascii_case(sender)) {
        return ("校园事务".into(), format!("校内来源：{}", mail.sender_email));
    }
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
    fn leaves_action_mail_for_manual_todo_tagging() {
        let mail = ParsedMail { uid: 1, sender_name: "A".into(), sender_email: "a@x.com".into(), recipients: vec![], subject: "请处理合同".into(), received_at: "".into(), body_text: "".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "外部");
    }
    #[test]
    fn classifies_internship_before_institutional_sender_signals() {
        let mail = ParsedMail { uid: 2, sender_name: "Innovation Centre".into(), sender_email: "ic@bnbu.edu.cn".into(), recipients: vec![], subject: "Summer internship application deadline".into(), received_at: "".into(), body_text: "Submit your resume by Friday".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "实习");
    }
    #[test]
    fn classifies_cdc_career_planning_seminar_as_campus_affairs() {
        let mail = ParsedMail { uid: 3, sender_name: "CDC".into(), sender_email: "career@bnbu.edu.cn".into(), recipients: vec![], subject: "【CDC宣讲会 | Career Session】职业发展规划讲座".into(), received_at: "".into(), body_text: "学生职业发展中心帮助同学建立职业规划思维。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn classifies_innovation_center_competition_event_as_campus_affairs() {
        let mail = ParsedMail { uid: 4, sender_name: "创新中心".into(), sender_email: "ic@bnbu.edu.cn".into(), recipients: vec![], subject: "2026釜涌杯暨国创赛BNBU校赛决赛项目打磨交流会报名通知".into(), received_at: "".into(), body_text: "创新中心将举办校赛决赛项目打磨交流会。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn classifies_maker_center_incubation_notice_as_campus_affairs() {
        let mail = ParsedMail { uid: 5, sender_name: "创客中心".into(), sender_email: "ic@bnbu.edu.cn".into(), recipients: vec![], subject: "BNBU创客中心入孵项目申报通知".into(), received_at: "".into(), body_text: "BNBU创客中心入孵项目申报已开启。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn classifies_invited_dls_lecture_as_campus_affairs() {
        let mail = ParsedMail { uid: 6, sender_name: "FST".into(), sender_email: "fst@bnbu.edu.cn".into(), recipients: vec![], subject: "Invitation to FST-DLS Lecture on 4 Sep".into(), received_at: "".into(), body_text: "诚挚邀请您参加FST-DLS系列讲座。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn keeps_ordinary_course_lecture_in_academics() {
        let mail = ParsedMail { uid: 7, sender_name: "Teacher".into(), sender_email: "teacher@bnbu.edu.cn".into(), recipients: vec![], subject: "Lecture 3 slides".into(), received_at: "".into(), body_text: "Materials for this week's course.".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "学业");
    }
    #[test]
    fn classifies_collection_of_cleared_personal_items_as_personal() {
        let mail = ParsedMail { uid: 8, sender_name: "SLCDT".into(), sender_email: "slcdt@bnbu.edu.cn".into(), recipients: vec![], subject: "科雅苑公共区域清理物品领取通知".into(), received_at: "".into(), body_text: "请同学们到舍堂助理处登记后认领物品。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "个人");
    }
    #[test]
    fn classifies_career_center_consultation_service_as_campus_affairs() {
        let mail = ParsedMail { uid: 9, sender_name: "Career Centre".into(), sender_email: "career@uic.edu.cn".into(), recipients: vec![], subject: "一对一咨询项目定制升级啦".into(), received_at: "".into(), body_text: "本学期一对一咨询项目现已正式开放预约。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn keeps_ordinary_appointment_as_personal() {
        let mail = ParsedMail { uid: 10, sender_name: "Clinic".into(), sender_email: "clinic@example.com".into(), recipients: vec![], subject: "Appointment confirmation".into(), received_at: "".into(), body_text: "Your appointment is confirmed.".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "个人");
    }
    #[test]
    fn classifies_student_hostel_safety_inspection_as_personal() {
        let mail = ParsedMail { uid: 11, sender_name: "SLCDT".into(), sender_email: "slcdt@bnbu.edu.cn".into(), recipients: vec![], subject: "关于开展9月学生宿舍安全检查的通知".into(), received_at: "".into(), body_text: "学生事务处将联合物业管理处开展学生宿舍安全专项检查。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "个人");
    }
    #[test]
    fn keeps_ordinary_hostel_facility_notice_in_campus_affairs() {
        let mail = ParsedMail { uid: 12, sender_name: "SLCDT".into(), sender_email: "slcdt@bnbu.edu.cn".into(), recipients: vec![], subject: "宿舍公共区域停水通知".into(), received_at: "".into(), body_text: "物业将检修供水设施。".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn classifies_university_datathon_notice_as_campus_affairs() {
        let mail = ParsedMail { uid: 13, sender_name: "BNBU FST".into(), sender_email: "fst@bnbu.edu.cn".into(), recipients: vec![], subject: "Smart Logistics Datathon 2026_Please reply this email if you have signed up".into(), received_at: "".into(), body_text: "Please reply with your team composition and registration information.".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
    #[test]
    fn classifies_guest_lecture_as_campus_affairs() {
        let mail = ParsedMail { uid: 14, sender_name: "DFE".into(), sender_email: "dfe@bnbu.edu.cn".into(), recipients: vec![], subject: "DFE Guest Lecture | Seasonal Inventory Leverage".into(), received_at: "".into(), body_text: "Sept. 9, 14:00-15:00, T1-302-R1".into(), is_read: false };
        assert_eq!(classify(&mail, &[]).0, "校园事务");
    }
}
