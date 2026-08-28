export const categories = ["待办", "学业", "校园事务", "社团活动", "个人", "外部"] as const;
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
  initialDays: number;
  syncMinutes: number;
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
