import { invoke } from "@tauri-apps/api/core";
import { demoMails } from "./demo";
import type { AssignmentDeadline, CalendarEvent, Category, ClassificationRule, MailMessage, MailSettings, PersonalReminderInput, ReminderDraft } from "./types";

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
  }
};
