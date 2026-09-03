import { invoke } from "@tauri-apps/api/core";
import { demoMails } from "./demo";
import type { AcademicCalendarImport, AssignmentDeadline, CalendarEvent, Category, ClassificationRule, MailIdentity, MailMessage, MailSettings, OutgoingMail, PersonalReminderInput, ProfessorContact, ReminderDraft, SystemReminderInput } from "./types";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export const api = {
  async listMails(): Promise<MailMessage[]> {
    return isTauri() ? invoke("list_mails") : structuredClone(demoMails);
  },
  async syncMails(): Promise<MailMessage[]> {
    if (!isTauri()) { await new Promise((r) => setTimeout(r, 700)); return structuredClone(demoMails); }
    await invoke("sync_mail");
    return invoke("list_mails");
  },
  async testConnection(settings: MailSettings, password: string): Promise<string> {
    if (!isTauri()) { await new Promise((r) => setTimeout(r, 500)); return "演示模式：配置格式有效"; }
    return invoke("test_mail_connection", { settings, password });
  },
  async saveSettings(settings: MailSettings, password: string): Promise<void> {
    if (!isTauri()) { localStorage.setItem("mail-settings", JSON.stringify(settings)); return; }
    await invoke("save_mail_settings", { settings, password });
  },
  async updateCategory(mailId: string, category: Category): Promise<void> {
    if (isTauri()) await invoke("update_mail_category", { mailId, category });
  },
  async updateTodo(mailId: string, isTodo: boolean): Promise<void> {
    if (isTauri()) await invoke("update_mail_todo", { mailId, isTodo });
  },
  async createReminder(draft: ReminderDraft): Promise<string> {
    if (!isTauri()) {
      await new Promise((r) => setTimeout(r, 500));
      const events = await this.listCalendarEvents();
      const event: CalendarEvent = { id: `mail:${draft.sourceMailId}`, title: draft.title, notes: draft.notes, startsAt: draft.dueAt, priority: draft.priority, kind: "mail", sourceId: draft.sourceMailId, sourceUrl: draft.sourceUrl, readOnly: true };
      localStorage.setItem("calendar-events", JSON.stringify([...events.filter((item) => item.id !== event.id), event]));
      return `demo-${Date.now()}`;
    }
    return invoke("create_reminder", { draft });
  },
  async listCalendarEvents(): Promise<CalendarEvent[]> {
    if (isTauri()) return invoke("list_calendar_events");
    try { return JSON.parse(localStorage.getItem("calendar-events") ?? "[]"); } catch { return []; }
  },
  async savePersonalReminder(input: PersonalReminderInput): Promise<CalendarEvent> {
    if (isTauri()) return invoke("save_personal_reminder", { input });
    const events = await this.listCalendarEvents();
    const event: CalendarEvent = { id: input.id ?? `personal:${crypto.randomUUID()}`, title: input.title, notes: input.notes, startsAt: input.startsAt, priority: input.priority, kind: "personal", readOnly: false };
    localStorage.setItem("calendar-events", JSON.stringify([...events.filter((item) => item.id !== event.id), event]));
    return event;
  },
  async deletePersonalReminder(eventId: string): Promise<void> {
    if (isTauri()) return invoke("delete_personal_reminder", { eventId });
    localStorage.setItem("calendar-events", JSON.stringify((await this.listCalendarEvents()).filter((item) => item.id !== eventId || item.kind !== "personal")));
  },
  async createSystemReminder(input: SystemReminderInput): Promise<string> {
    if (isTauri()) return invoke("create_system_reminder", { input });
    const sources = await this.listSystemReminderSources();
    if (sources.includes(input.sourceId)) throw new Error("这个事项已经加入过电脑提醒");
    localStorage.setItem("system-reminder-sources", JSON.stringify([...sources, input.sourceId]));
    return `demo-system-${Date.now()}`;
  },
  async listSystemReminderSources(): Promise<string[]> {
    if (isTauri()) return invoke("list_system_reminder_sources");
    try { return JSON.parse(localStorage.getItem("system-reminder-sources") ?? "[]"); } catch { return []; }
  },
  async clearLocalData(): Promise<void> {
    if (isTauri()) await invoke("clear_local_data");
  },
  async listRules(): Promise<ClassificationRule[]> {
    if (isTauri()) return invoke("list_rules");
    try { return JSON.parse(localStorage.getItem("classification-rules") ?? "[]"); } catch { return []; }
  },
  async saveRule(rule: ClassificationRule): Promise<void> {
    if (isTauri()) return invoke("save_rule", { rule });
    const rules = await this.listRules();
    localStorage.setItem("classification-rules", JSON.stringify([...rules.filter((r) => r.id !== rule.id), rule]));
  },
  async deleteRule(ruleId: string): Promise<void> {
    if (isTauri()) return invoke("delete_rule", { ruleId });
    localStorage.setItem("classification-rules", JSON.stringify((await this.listRules()).filter((r) => r.id !== ruleId)));
  },
  async listAssignments(): Promise<AssignmentDeadline[]> {
    return isTauri() ? invoke("list_assignments") : [];
  },
  async saveIspaceCalendarUrl(calendarUrl: string): Promise<void> {
    if (isTauri()) await invoke("save_ispace_calendar_url", { calendarUrl });
  },
  async syncAssignments(): Promise<AssignmentDeadline[]> {
    if (!isTauri()) return [];
    await invoke("sync_assignments");
    return invoke("list_assignments");
  },
  async importAcademicCalendar(input: AcademicCalendarImport): Promise<number> {
    if (isTauri()) return invoke("import_academic_calendar", { input });
    const events = await this.listCalendarEvents();
    const imported = parseAcademicEntries(input);
    localStorage.setItem("calendar-events", JSON.stringify([...events.filter((item) => item.kind !== "academic" || item.sourceId !== input.semester.trim()), ...imported]));
    return imported.length;
  },
  async sendMail(outgoing: OutgoingMail): Promise<string> {
    if (!isTauri()) { await new Promise((resolve) => setTimeout(resolve, 700)); return "演示模式：邮件发送成功"; }
    const payload = { ...outgoing, attachments: outgoing.attachments.map(({ size: _size, ...attachment }) => attachment) };
    return invoke("send_mail", { outgoing: payload });
  },
  async getMailIdentity(): Promise<MailIdentity> {
    if (!isTauri()) return { email: "student@example.edu", displayName: "Student Name" };
    return invoke("get_mail_identity");
  },
  async translateMail(text: string): Promise<string> {
    if (!isTauri()) { await new Promise((resolve) => setTimeout(resolve, 500)); return "这是英文邮件的中文翻译预览。"; }
    return invoke("translate_mail", { text, consent: true });
  },
  async searchProfessors(query: string): Promise<ProfessorContact[]> {
    if (!isTauri()) return [{ name: "Prof. Weimin LIU", email: "weiminliu@bnbu.edu.cn", department: "BNBU 官网演示结果", sourceUrl: "https://www.bnbu.edu.cn/en/" }];
    return invoke("search_professors", { query });
  }
};

function parseAcademicEntries(input: AcademicCalendarImport): CalendarEvent[] {
  return input.entries.trim().split(/\r?\n/).flatMap((line, lineIndex) => {
    const [range, title] = line.split("|").map((value) => value.trim());
    if (!range || !title) throw new Error(`第 ${lineIndex + 1} 行格式错误`);
    const [startText, endText = startText] = range.split("~").map((value) => value.trim());
    const start = new Date(`${startText}T09:00:00+08:00`);
    const end = new Date(`${endText}T09:00:00+08:00`);
    if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime()) || end < start) throw new Error(`第 ${lineIndex + 1} 行日期无效`);
    const output: CalendarEvent[] = [];
    for (const date = new Date(start); date <= end; date.setDate(date.getDate() + 1)) output.push({ id: `academic:${input.semester}:${date.toISOString()}:${lineIndex}`, title, notes: input.semester, startsAt: date.toISOString(), priority: "normal", kind: "academic", sourceId: input.semester, sourceUrl: input.sourceUrl, readOnly: true });
    return output;
  });
}
