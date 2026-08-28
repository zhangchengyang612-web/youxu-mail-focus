import { describe, expect, it } from "vitest";
import { classifyMail, createReminderDraft, extractDueDate, summarizeMail } from "./domain";
import type { MailMessage } from "./types";

const mail: MailMessage = { id: "1", uid: 1, folder: "INBOX", senderName: "课程助教", senderEmail: "ta@bnbu.edu.cn", recipients: [], subject: "课程作业提醒", receivedAt: "2026-08-28T09:00:00Z", bodyText: "请在明天 10:30 前完成作业。", category: "学业", classificationReason: "", isRead: false };

describe("classification", () => {
  it("uses custom rules before built-ins", () => expect(classifyMail(mail, [{ id: "r", category: "校园事务", field: "sender", operator: "contains", value: "bnbu", priority: 99, enabled: true }]).category).toBe("校园事务"));
  it("uses built-in keywords", () => expect(classifyMail(mail).category).toBe("学业"));
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
