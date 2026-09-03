import { describe, expect, it } from "vitest";
import { buildMonthGrid, classifyMail, createReminderDraft, extractDueDate, localDateKey, summarizeMail } from "./domain";
import type { MailMessage } from "./types";

const mail: MailMessage = { id: "1", uid: 1, folder: "INBOX", senderName: "课程助教", senderEmail: "ta@bnbu.edu.cn", recipients: [], subject: "课程作业提醒", receivedAt: "2026-08-28T09:00:00Z", bodyText: "请在明天 10:30 前完成作业。", category: "学业", classificationReason: "", isTodo: false, isRead: false };

describe("classification", () => {
  it("uses custom rules before built-ins", () => expect(classifyMail(mail, [{ id: "r", category: "校园事务", field: "sender", operator: "contains", value: "bnbu", priority: 99, enabled: true }]).category).toBe("校园事务"));
  it("uses built-in keywords", () => expect(classifyMail(mail).category).toBe("学业"));
  it("classifies internship before institutional sender signals", () => expect(classifyMail({ ...mail, senderEmail: "ic@bnbu.edu.cn", subject: "Summer internship application deadline", bodyText: "Submit your resume by Friday" }).category).toBe("实习"));
  it("does not assign the manual todo label", () => expect(classifyMail({ ...mail, subject: "请完成合同", bodyText: "action required" }).category).toBe("外部"));
  it("classifies a CDC career planning seminar as campus affairs", () => expect(classifyMail({ ...mail, senderEmail: "career@bnbu.edu.cn", subject: "【CDC宣讲会 | Career Session】职业发展规划讲座", bodyText: "学生职业发展中心帮助同学建立职业规划思维。" }).category).toBe("校园事务"));
  it("classifies an innovation center campus competition event as campus affairs", () => expect(classifyMail({ ...mail, senderEmail: "ic@bnbu.edu.cn", subject: "2026釜涌杯暨国创赛BNBU校赛决赛项目打磨交流会报名通知", bodyText: "创新中心将举办校赛决赛项目打磨交流会。" }).category).toBe("校园事务"));
  it("classifies a maker center incubation notice as campus affairs", () => expect(classifyMail({ ...mail, senderEmail: "ic@bnbu.edu.cn", subject: "BNBU创客中心入孵项目申报通知", bodyText: "BNBU创客中心入孵项目申报已开启。" }).category).toBe("校园事务"));
  it("classifies an invited DLS lecture as campus affairs", () => expect(classifyMail({ ...mail, senderEmail: "fst@bnbu.edu.cn", subject: "Invitation to FST-DLS Lecture on 4 Sep", bodyText: "诚挚邀请您参加FST-DLS系列讲座。" }).category).toBe("校园事务"));
  it("keeps an ordinary course lecture in academics", () => expect(classifyMail({ ...mail, subject: "Lecture 3 slides", bodyText: "Materials for this week's course." }).category).toBe("学业"));
  it("classifies collection of cleared personal items as personal", () => expect(classifyMail({ ...mail, senderEmail: "slcdt@bnbu.edu.cn", subject: "科雅苑公共区域清理物品领取通知", bodyText: "请同学们到舍堂助理处登记后认领物品。" }).category).toBe("个人"));
  it("classifies a career center consultation service as campus affairs", () => expect(classifyMail({ ...mail, senderEmail: "career@uic.edu.cn", subject: "一对一咨询项目定制升级啦", bodyText: "本学期一对一咨询项目现已正式开放预约。" }).category).toBe("校园事务"));
  it("keeps an ordinary appointment as personal", () => expect(classifyMail({ ...mail, senderEmail: "clinic@example.com", subject: "Appointment confirmation", bodyText: "Your appointment is confirmed." }).category).toBe("个人"));
  it("classifies a student hostel safety inspection as personal", () => expect(classifyMail({ ...mail, senderEmail: "slcdt@bnbu.edu.cn", subject: "关于开展9月学生宿舍安全检查的通知", bodyText: "学生事务处将联合物业管理处开展学生宿舍安全专项检查。" }).category).toBe("个人"));
  it("keeps an ordinary hostel facility notice in campus affairs", () => expect(classifyMail({ ...mail, senderEmail: "slcdt@bnbu.edu.cn", subject: "宿舍公共区域停水通知", bodyText: "物业将检修供水设施。" }).category).toBe("校园事务"));
  it("classifies a university datathon notice as campus affairs", () => expect(classifyMail({ ...mail, senderName: "BNBU FST", subject: "Smart Logistics Datathon 2026_Please reply this email if you have signed up", bodyText: "Please reply with your team composition and registration information." }).category).toBe("校园事务"));
  it("classifies a guest lecture as campus affairs", () => expect(classifyMail({ ...mail, senderName: "DFE", subject: "DFE Guest Lecture | Seasonal Inventory Leverage", bodyText: "Sept. 9, 14:00-15:00, T1-302-R1" }).category).toBe("校园事务"));
});

describe("reminder extraction", () => {
  it("extracts Chinese relative dates", () => expect(extractDueDate(mail.bodyText, new Date("2026-08-28T00:00:00+08:00"))).toContain("2026-08-29"));
  it("creates editable draft", () => expect(createReminderDraft(mail)).toMatchObject({ title: "课程作业提醒", priority: "high", sourceMailId: "1" }));
});

describe("local mail summary", () => {
  it("prioritizes action and deadline sentences", () => {
    const summary = summarizeMail({
      subject: "课程作业安排",
      bodyText: "同学们好。这里是本周课程的一般说明，内容较长。请在2026年9月2日前提交作业。逾期将无法补交。谢谢。",
    }, 80);
    expect(summary).toContain("提交作业");
    expect(summary.length).toBeLessThanOrEqual(81);
  });

  it("removes quoted reply headers", () => {
    expect(summarizeMail({ subject: "提醒", bodyText: "请明天参加会议。\n发件人：旧邮件\n> 历史引用内容" })).toBe("请明天参加会议。");
  });
});

describe("calendar", () => {
  it("builds a six-week Monday-first month grid", () => {
    const grid = buildMonthGrid(new Date(2026, 7, 1));
    expect(grid).toHaveLength(42);
    expect(grid[0].getDay()).toBe(1);
    expect(localDateKey(grid[0])).toBe("2026-07-27");
  });

  it("creates stable local date keys", () => {
    expect(localDateKey(new Date(2026, 7, 29, 18, 30))).toBe("2026-08-29");
  });
});
