import type { Category, ClassificationRule, MailMessage, ReminderDraft } from "./types";

const defaults: Record<Exclude<Category, "外部">, string[]> = {
  "实习": ["实习", "实习生", "校招", "招聘", "简历", "internship", "intern position", "recruitment", "student assistant", "career opportunity", "job opening", "resume"],
  "学业": ["课程", "作业", "考试", "成绩", "课堂", "选课", "论文", "导师", "assignment", "course", "exam", "quiz", "lecture"],
  "校园事务": ["教务", "校园", "宿舍", "图书馆", "注册", "缴费", "学生事务", "itsc", "校园卡", "系统通知", "学生职业发展中心", "职业发展规划", "cdc宣讲会", "career development center", "career development centre", "一对一咨询项目", "创新中心", "创客中心", "入孵项目", "校赛", "项目打磨交流会"],
  "社团活动": ["社团", "活动", "招募", "志愿者", "比赛", "报名", "讲座", "工作坊", "club", "event", "workshop"],
  "个人": ["个人", "预约", "账单", "快递", "账户", "生日", "appointment", "personal"]
};
const campusEventPhrases = ["系列讲座", "dls lecture", "guest lecture", "datathon"];
const personalItemPhrases = ["物品领取", "物品认领", "认领物品", "宿舍安全检查", "宿舍安全专项检查", "student hostel safety inspection"];
const campusSenders = ["career@uic.edu.cn", "career@bnbu.edu.cn", "ic@bnbu.edu.cn"];

function matches(rule: ClassificationRule, mail: MailMessage): boolean {
  if (!rule.enabled) return false;
  const source = rule.field === "sender" ? mail.senderEmail
    : rule.field === "domain" ? mail.senderEmail.split("@")[1] ?? ""
    : rule.field === "subject" ? mail.subject : mail.bodyText;
  try {
    if (rule.operator === "equals") return source.toLowerCase() === rule.value.toLowerCase();
    if (rule.operator === "regex") return new RegExp(rule.value, "i").test(source);
    return source.toLowerCase().includes(rule.value.toLowerCase());
  } catch { return false; }
}

export function classifyMail(mail: MailMessage, rules: ClassificationRule[] = []): Pick<MailMessage, "category" | "classificationReason"> {
  const rule = [...rules].sort((a, b) => b.priority - a.priority).find((item) => matches(item, mail));
  if (rule) return { category: rule.category, classificationReason: `规则：${rule.field} ${rule.operator} ${rule.value}` };
  const haystack = `${mail.subject}\n${mail.bodyText}`.toLowerCase();
  const internship = defaults["实习"].find((item) => haystack.includes(item));
  if (internship) return { category: "实习", classificationReason: `关键词：${internship}` };
  const personalItem = personalItemPhrases.find((item) => haystack.includes(item));
  if (personalItem) return { category: "个人", classificationReason: `关键词：${personalItem}` };
  const campusEvent = campusEventPhrases.find((item) => haystack.includes(item));
  if (campusEvent) return { category: "校园事务", classificationReason: `关键词：${campusEvent}` };
  if (campusSenders.includes(mail.senderEmail.toLowerCase())) return { category: "校园事务", classificationReason: `校内来源：${mail.senderEmail}` };
  for (const [category, words] of Object.entries(defaults) as [Exclude<Category, "外部">, string[]][]) {
    const word = words.find((item) => haystack.includes(item.toLowerCase()));
    if (word) return { category, classificationReason: `关键词：${word}` };
  }
  return { category: "外部", classificationReason: "未匹配校内或个人规则" };
}

export function extractDueDate(text: string, now = new Date()): string | null {
  const normalized = text.replace(/[年/.]/g, "-").replace(/月/g, "-").replace(/日/g, " ");
  const absolute = normalized.match(/(20\d{2})-(\d{1,2})-(\d{1,2})(?:\s+(\d{1,2})(?::(\d{2}))?)?/);
  if (absolute) {
    const [, y, m, d, h = "18", min = "00"] = absolute;
    const date = new Date(Number(y), Number(m) - 1, Number(d), Number(h), Number(min));
    return Number.isNaN(date.getTime()) ? null : date.toISOString();
  }
  const relative = text.match(/(今天|明天|后天)(?:\s*(\d{1,2})(?::|点)(\d{2})?)?/);
  if (relative) {
    const offset = relative[1] === "今天" ? 0 : relative[1] === "明天" ? 1 : 2;
    const date = new Date(now);
    date.setDate(date.getDate() + offset);
    date.setHours(Number(relative[2] ?? 18), Number(relative[3] ?? 0), 0, 0);
    return date.toISOString();
  }
  const english = text.match(/\b(today|tomorrow)\b(?:\s+at\s+(\d{1,2})(?::(\d{2}))?)?/i);
  if (english) {
    const date = new Date(now);
    if (english[1].toLowerCase() === "tomorrow") date.setDate(date.getDate() + 1);
    date.setHours(Number(english[2] ?? 18), Number(english[3] ?? 0), 0, 0);
    return date.toISOString();
  }
  return null;
}

export function createReminderDraft(mail: MailMessage): ReminderDraft {
  const actionable = mail.bodyText.split(/\n|。|！|!/).map((s) => s.trim()).find((s) => /请|需|务必|please|must|截止/i.test(s));
  return {
    title: mail.subject.replace(/^(re|fw|fwd):\s*/i, "").slice(0, 120),
    notes: [`来自：${mail.senderName} <${mail.senderEmail}>`, actionable || mail.bodyText.slice(0, 280), `邮件时间：${new Date(mail.receivedAt).toLocaleString("zh-CN")}`].join("\n\n"),
    dueAt: extractDueDate(`${mail.subject}\n${mail.bodyText}`),
    priority: mail.isTodo || mail.category === "学业" ? "high" : "normal",
    sourceMailId: mail.id
  };
}

export function summarizeMail(mail: Pick<MailMessage, "subject" | "bodyText">, maxLength = 260): string {
  const cleaned = mail.bodyText
    .replace(/\r/g, "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !/^>/.test(line))
    .filter((line) => !/^(from|sent|to|subject|发件人|发送时间|收件人|主题)\s*[:：]/i.test(line))
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();

  if (!cleaned) return "这封邮件没有可供摘要的正文。";
  if (cleaned.length <= maxLength) return cleaned;

  const sentences = (cleaned.match(/[^。！？!?；;\.]+[。！？!?；;\.]?/g) ?? [cleaned])
    .map((text) => text.trim())
    .filter((text) => text.length >= 8);
  const subjectTerms = mail.subject.toLowerCase().match(/[\p{L}\p{N}]{2,}/gu) ?? [];
  const important = /请|需要|务必|截止|提交|完成|报名|时间|地点|通知|提醒|please|must|deadline|due|submit|required|schedule/i;
  const dated = /\d{1,4}[年/月日.\-:]|今天|明天|后天|周[一二三四五六日天]|星期|today|tomorrow|monday|tuesday|wednesday|thursday|friday/i;

  const selected = sentences
    .map((text, index) => ({
      text,
      index,
      score: (index === 0 ? 3 : 0)
        + (important.test(text) ? 5 : 0)
        + (dated.test(text) ? 4 : 0)
        + subjectTerms.filter((term) => text.toLowerCase().includes(term)).length,
    }))
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .slice(0, 3)
    .sort((a, b) => a.index - b.index)
    .map(({ text }) => text)
    .join(" ");

  const summary = selected || cleaned;
  return summary.length > maxLength ? `${summary.slice(0, maxLength).trimEnd()}…` : summary;
}

export function localDateKey(value: Date | string): string {
  const date = typeof value === "string" ? new Date(value) : value;
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function buildMonthGrid(month: Date): Date[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const mondayOffset = (first.getDay() + 6) % 7;
  const start = new Date(first);
  start.setDate(first.getDate() - mondayOffset);
  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start);
    date.setDate(start.getDate() + index);
    return date;
  });
}
