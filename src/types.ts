export const categories = ["学业", "校园事务", "社团活动", "实习", "个人", "外部"] as const;
export type Category = (typeof categories)[number];

export interface MailMessage {
  id: string;
  uid: number;
  folder: string;
  senderName: string;
  senderEmail: string;
  recipients: string[];
  subject: string;
  receivedAt: string;
  bodyText: string;
  category: Category;
  isTodo: boolean;
  classificationReason: string;
  isRead: boolean;
  reminderStatus?: "none" | "draft" | "created" | "failed";
}

export interface ClassificationRule {
  id: string;
  category: Category;
  field: "sender" | "domain" | "subject" | "body";
  operator: "contains" | "equals" | "regex";
  value: string;
  priority: number;
  enabled: boolean;
}

export interface ReminderDraft {
  title: string;
  notes: string;
  dueAt: string | null;
  priority: "low" | "normal" | "high";
  sourceMailId: string;
  sourceUrl?: string;
}

export interface MailSettings {
  host: string;
  port: number;
  email: string;
  senderName: string;
  initialDays: number;
  syncMinutes: number;
}

export interface MailIdentity {
  email: string;
  displayName: string;
}

export interface AssignmentDeadline {
  id: string;
  title: string;
  course: string;
  dueAt: string;
  description: string;
  url?: string;
  source: "iSpace";
}

export type CalendarEventKind = "ispace" | "mail" | "personal" | "academic";

export interface CalendarEvent {
  id: string;
  title: string;
  notes: string;
  startsAt: string | null;
  priority: "low" | "normal" | "high";
  kind: CalendarEventKind;
  sourceId?: string;
  sourceUrl?: string;
  readOnly: boolean;
}

export interface PersonalReminderInput {
  id?: string;
  title: string;
  notes: string;
  startsAt: string;
  priority: "low" | "normal" | "high";
}

export interface SystemReminderInput {
  sourceId: string;
  title: string;
  notes: string;
  dueAt: string;
  priority: "low" | "normal" | "high";
}

export interface AcademicCalendarImport {
  semester: string;
  sourceUrl: string;
  entries: string;
}

export interface OutgoingAttachment {
  name: string;
  mimeType: string;
  dataBase64: string;
  size: number;
}

export interface OutgoingMail {
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  textBody: string;
  htmlBody: string;
  attachments: OutgoingAttachment[];
}

export interface ProfessorContact {
  name: string;
  email: string;
  department: string;
  sourceUrl: string;
}
