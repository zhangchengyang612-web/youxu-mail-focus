import { useEffect, useMemo, useState } from "react";
import { Bell, BookOpen, Building2, CalendarClock, Check, ChevronDown, Clock3, Copy, Globe2, GraduationCap, Inbox, ListTodo, Menu, MoreHorizontal, PartyPopper, RefreshCw, Search, Settings, ShieldCheck, SlidersHorizontal, Sparkles, Trash2, UserRound, X } from "lucide-react";
import { api } from "./api";
import { createReminderDraft, summarizeMail } from "./domain";
import { categories, type AssignmentDeadline, type Category, type ClassificationRule, type MailMessage, type MailSettings, type ReminderDraft } from "./types";

const icons: Record<Category | "全部邮件", typeof Inbox> = { "全部邮件": Inbox, "待办": ListTodo, "学业": BookOpen, "校园事务": Building2, "社团活动": PartyPopper, "个人": UserRound, "外部": Globe2 };
const colors: Record<Category, string> = { "待办": "coral", "学业": "violet", "校园事务": "amber", "社团活动": "green", "个人": "blue", "外部": "gray" };

const defaultSettings: MailSettings = { host: "imap.exmail.qq.com", port: 993, email: "", initialDays: 0, syncMinutes: 5 };

export default function App() {
  const [mails, setMails] = useState<MailMessage[]>([]);
  const [category, setCategory] = useState<Category | "全部邮件">("全部邮件");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [syncing, setSyncing] = useState(false);
  const [status, setStatus] = useState("正在读取本地邮件…");
  const [showSettings, setShowSettings] = useState(false);
  const [showRules, setShowRules] = useState(false);
  const [draft, setDraft] = useState<ReminderDraft | null>(null);
  const [expandedMails, setExpandedMails] = useState<Set<string>>(new Set());
  const [toast, setToast] = useState<string | null>(null);
  const [showAssignments, setShowAssignments] = useState(false);
  const [assignments, setAssignments] = useState<AssignmentDeadline[]>([]);
  const [ispaceStatus, setIspaceStatus] = useState("正在读取本地 DDL…");
  const [showIspaceSettings, setShowIspaceSettings] = useState(false);

  useEffect(() => {
    api.listMails().then((items) => { setMails(items); setSelectedId(items[0]?.id ?? null); setStatus("本地数据已就绪"); });
    api.listAssignments().then((items) => { setAssignments(items); setIspaceStatus(items.length ? `本地已有 ${items.length} 项待提交作业` : "尚未同步 iSpace 作业"); });
    if (!("__TAURI_INTERNALS__" in window)) return;
    let minutes = 5; try { minutes = JSON.parse(localStorage.getItem("mail-settings") ?? "null")?.syncMinutes ?? 5; } catch { /* default */ }
    const timer = window.setInterval(() => { void sync(); }, Math.max(1, minutes) * 60_000);
    return () => window.clearInterval(timer);
  }, []);
  const filtered = useMemo(() => mails.filter((mail) => (category === "全部邮件" || mail.category === category) && `${mail.subject} ${mail.senderName} ${mail.bodyText}`.toLowerCase().includes(query.toLowerCase())), [mails, category, query]);
  const selected = mails.find((mail) => mail.id === selectedId) ?? filtered[0];

  async function sync() {
    setSyncing(true); setStatus("正在安全同步…");
    try { const items = await api.syncMails(); setMails(items); setStatus(`刚刚同步 · ${items.length} 封邮件`); }
    catch (e) { setStatus(`同步失败：${String(e)}`); }
    try { const items = await api.syncAssignments(); setAssignments(items); setIspaceStatus(`刚刚同步 · ${items.length} 项待提交作业`); }
    catch (e) { if (!String(e).includes("尚未配置")) setIspaceStatus(`iSpace 同步失败：${String(e)}`); }
    finally { setSyncing(false); }
  }
  async function syncIspace() {
    setSyncing(true); setIspaceStatus("正在同步 iSpace DDL…");
    try { const items = await api.syncAssignments(); setAssignments(items); setIspaceStatus(`刚刚同步 · ${items.length} 项待提交作业`); }
    catch (e) { setIspaceStatus(`同步失败：${String(e)}`); }
    finally { setSyncing(false); }
  }
  function toggle(id: string) { setChecked((old) => { const next = new Set(old); next.has(id) ? next.delete(id) : next.add(id); return next; }); }
  function openReminder() {
    const mail = mails.find((item) => checked.has(item.id)) ?? selected;
    if (mail) setDraft(createReminderDraft(mail));
  }
  async function changeCategory(next: Category) {
    if (!selected) return;
    setMails((old) => old.map((item) => item.id === selected.id ? { ...item, category: next, classificationReason: "手动分类" } : item));
    await api.updateCategory(selected.id, next);
  }

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Sparkles size={18}/></div><div><strong>邮序</strong><span>MAIL FOCUS</span></div></div>
      <button className="compose" onClick={openReminder}><Bell size={17}/>创建提醒</button>
      <nav>
        <p className="nav-label">邮件</p>
        {(["全部邮件", ...categories] as const).map((item) => { const Icon = icons[item]; const count = item === "全部邮件" ? mails.length : mails.filter((mail) => mail.category === item).length; return <button key={item} className={!showAssignments && category === item ? "nav-item active" : "nav-item"} onClick={() => { setShowAssignments(false); setCategory(item); }}><Icon size={17}/><span>{item}</span><em>{count}</em></button>; })}
        <p className="nav-label nav-space">iSpace</p>
        <button className={showAssignments ? "nav-item active" : "nav-item"} onClick={() => setShowAssignments(true)}><CalendarClock size={17}/><span>作业 DDL</span><em>{assignments.length}</em></button>
      </nav>
      <div className="privacy"><ShieldCheck size={18}/><div><strong>数据仅存本机</strong><span>正文不会上传云端</span></div></div>
      <button className="settings-button" onClick={() => setShowRules(true)}><SlidersHorizontal size={17}/>分类规则</button>
      <button className="settings-button" onClick={() => setShowIspaceSettings(true)}><GraduationCap size={17}/>连接 iSpace</button>
      <button className="settings-button" onClick={() => setShowSettings(true)}><Settings size={17}/>设置</button>
    </aside>

    <main className="workspace">
      <header className="topbar">
        <div className="mobile-menu"><Menu/></div>
        <div className="search"><Search size={17}/><input aria-label="搜索" placeholder={showAssignments ? "搜索课程或作业…" : "搜索发件人、主题或正文…"} value={query} onChange={(e) => setQuery(e.target.value)}/><kbd>⌘ K</kbd></div>
        <button className="icon-button" onClick={showAssignments ? syncIspace : sync} title={showAssignments ? "同步 iSpace 作业" : "同步邮件"}><RefreshCw size={18} className={syncing ? "spin" : ""}/></button>
        <div className="avatar">CY</div>
      </header>
      <div className="content">
        {showAssignments ? <AssignmentBoard assignments={assignments} status={ispaceStatus} query={query} syncing={syncing} onSync={syncIspace} onSetup={() => setShowIspaceSettings(true)} onToast={setToast}/> : <>
        <section className="mail-column">
          <div className="list-heading"><div><h1>{category}</h1><p>{status}</p></div><button className="filter">最新优先 <ChevronDown size={14}/></button></div>
          {checked.size > 0 && <div className="selection-bar"><span>已选择 {checked.size} 封</span><button onClick={openReminder}><Bell size={14}/>生成提醒</button><button className="plain" onClick={() => setChecked(new Set())}>取消</button></div>}
          <div className="mail-list">{filtered.map((mail) => <article key={mail.id} className={`mail-card ${selected?.id === mail.id ? "selected" : ""} ${!mail.isRead ? "unread" : ""}`} onClick={() => setSelectedId(mail.id)}>
            <button className={`check ${checked.has(mail.id) ? "checked" : ""}`} aria-label="选择邮件" onClick={(e) => { e.stopPropagation(); toggle(mail.id); }}>{checked.has(mail.id) && <Check size={12}/>}</button>
            <div className="mail-copy"><div className="mail-meta"><strong>{mail.senderName}</strong><time>{new Date(mail.receivedAt).toLocaleDateString("zh-CN", { month: "short", day: "numeric" })}</time></div><h3>{mail.subject}</h3><p>{summarizeMail(mail, 100)}</p><div className="tags"><span className={`tag ${colors[mail.category]}`}>{mail.category}</span>{mail.reminderStatus === "created" && <span className="reminded"><Check size={11}/>已创建提醒</span>}</div></div>
          </article>)}</div>
        </section>

        <section className="detail-column">{selected ? <>
          <div className="detail-toolbar"><button onClick={openReminder}><Bell size={16}/>生成提醒</button><button className="icon-button"><MoreHorizontal size={18}/></button></div>
          <article className="mail-detail"><span className={`tag ${colors[selected.category]}`}>{selected.category}</span><h2>{selected.subject}</h2><div className="sender-row"><div className="sender-avatar">{selected.senderName.slice(0, 1)}</div><div><strong>{selected.senderName}</strong><span>{selected.senderEmail}</span></div><time>{new Date(selected.receivedAt).toLocaleString("zh-CN")}</time></div><section className="mail-summary"><div className="summary-heading"><div><span>本地邮件摘要</span><strong>重点内容</strong></div><button onClick={() => setExpandedMails((old) => { const next = new Set(old); next.has(selected.id) ? next.delete(selected.id) : next.add(selected.id); return next; })}>{expandedMails.has(selected.id) ? "收起" : "展开"}</button></div><p>{summarizeMail(selected)}</p></section>{expandedMails.has(selected.id) && <div className="message-body"><div className="full-mail-label">完整邮件内容</div>{selected.bodyText.split("\n").map((line, i) => <p key={i}>{line || <br/>}</p>)}</div>}
          <div className="classification"><Sparkles size={17}/><div><strong>本地智能分类</strong><span>{selected.classificationReason}</span></div><select value={selected.category} onChange={(e) => changeCategory(e.target.value as Category)}>{categories.map((item) => <option key={item}>{item}</option>)}</select></div></article>
        </> : <div className="empty">选择一封邮件查看详情</div>}</section>
        </>}
      </div>
    </main>
    {showSettings && <SettingsModal onClose={() => setShowSettings(false)} onToast={setToast} onSaved={sync}/>} 
    {showIspaceSettings && <IspaceSettingsModal onClose={() => setShowIspaceSettings(false)} onToast={setToast} onSaved={syncIspace}/>} 
    {showRules && <RulesModal onClose={() => setShowRules(false)} onToast={setToast}/>} 
    {draft && <ReminderModal draft={draft} onClose={() => setDraft(null)} onCreated={(mailId) => { setMails((old) => old.map((m) => m.id === mailId ? {...m, reminderStatus: "created"} : m)); setChecked(new Set()); setDraft(null); setToast("提醒已创建"); }}/>} 
    {toast && <div className="toast" onAnimationEnd={() => setToast(null)}><Check size={16}/>{toast}</div>}
  </div>;
}

function AssignmentBoard({ assignments, status, query, syncing, onSync, onSetup, onToast }: { assignments: AssignmentDeadline[]; status: string; query: string; syncing: boolean; onSync: () => Promise<void>; onSetup: () => void; onToast: (value: string) => void }) {
  const visible = assignments.filter((item) => `${item.title} ${item.course} ${item.description}`.toLowerCase().includes(query.toLowerCase()));
  return <section className="assignment-board"><div className="assignment-header"><div><div className="modal-kicker">BNBU iSpace</div><h1>作业 DDL</h1><p>{status}</p></div><div className="assignment-actions"><button className="secondary" onClick={onSetup}><Settings size={15}/>订阅设置</button><button className="primary" disabled={syncing} onClick={onSync}><RefreshCw size={15} className={syncing ? "spin" : ""}/>{syncing ? "同步中…" : "同步 DDL"}</button></div></div><div className="deadline-overview"><div><strong>{assignments.length}</strong><span>待提交</span></div><div><strong>{assignments.filter((item) => daysUntil(item.dueAt) <= 7).length}</strong><span>7 天内截止</span></div><div><strong>{assignments.filter((item) => daysUntil(item.dueAt) <= 1).length}</strong><span>24 小时内</span></div></div>{visible.length ? <div className="assignment-grid">{visible.map((item) => { const days = daysUntil(item.dueAt); return <article className={`assignment-card ${days <= 1 ? "urgent" : days <= 7 ? "soon" : ""}`} key={item.id}><div className="deadline-date"><strong>{new Date(item.dueAt).toLocaleDateString("zh-CN", { month: "short", day: "numeric" })}</strong><span>{new Date(item.dueAt).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}</span></div><div className="assignment-copy"><span className="course-name">{item.course}</span><h2>{item.title}</h2>{item.description && <p>{item.description.slice(0, 180)}</p>}<div className="assignment-footer"><span><Clock3 size={13}/>{deadlineLabel(days)}</span>{item.url && <button onClick={async () => { await navigator.clipboard.writeText(item.url!); onToast("提交链接已复制"); }}><Copy size={13}/>复制提交链接</button>}</div></div></article>; })}</div> : <div className="assignment-empty"><CalendarClock size={36}/><h2>还没有待提交作业</h2><p>连接 iSpace 日历后，这里会集中显示 Assignment、Quiz、Turnitin 等截止时间。</p><button className="primary" onClick={onSetup}>连接 iSpace 日历</button></div>}</section>;
}

function daysUntil(value: string) { return Math.max(0, (new Date(value).getTime() - Date.now()) / 86_400_000); }
function deadlineLabel(days: number) { if (days < 1) return "24 小时内截止"; if (days < 2) return "明天截止"; return `${Math.ceil(days)} 天后截止`; }

function IspaceSettingsModal({ onClose, onToast, onSaved }: { onClose: () => void; onToast: (value: string) => void; onSaved: () => Promise<void> }) {
  const [calendarUrl, setCalendarUrl] = useState(""); const [saving, setSaving] = useState(false); const [result, setResult] = useState("");
  return <div className="overlay" onMouseDown={onClose}><div className="modal settings-modal" onMouseDown={(event) => event.stopPropagation()}><button className="close" onClick={onClose}><X/></button><div className="modal-kicker">个人日历订阅 · 直接 HTTPS 连接</div><h2>连接 BNBU iSpace</h2><p className="muted">程序直接读取你本人的 Moodle 日历订阅，不启动 VPN，也不需要管理员权限。请登录 iSpace，在“日历 → 导出日历”选择课程相关事件和未来范围，然后复制“获取日历 URL”生成的地址。</p><ol className="setup-steps"><li>打开 iSpace 日历并点击“导出日历”。</li><li>选择“课程相关事件”和可用的最长未来范围。</li><li>点击“获取日历 URL”，将地址粘贴到下方。</li><li>程序将通过 HTTPS 直接同步作业截止时间。</li></ol><label>iSpace 日历订阅 URL<input type="password" value={calendarUrl} onChange={(event) => setCalendarUrl(event.target.value)} placeholder="https://ispace.bnbu.edu.cn/calendar/export_execute.php?…"/></label><p className="secret-note"><ShieldCheck size={14}/>URL 含个人访问令牌，只保存到系统钥匙串，不写入数据库或日志。</p>{result && <div className="connection-result">{result}</div>}<div className="modal-actions"><button className="secondary" onClick={onClose}>取消</button><button className="primary" disabled={!calendarUrl.trim() || saving} onClick={async () => { setSaving(true); setResult("正在通过 HTTPS 直接读取作业 DDL…"); try { await api.saveIspaceCalendarUrl(calendarUrl); await onSaved(); onToast("iSpace 作业 DDL 已同步"); onClose(); } catch (error) { setResult(`连接失败：${String(error)}`); } finally { setSaving(false); } }}>{saving ? "正在连接…" : "保存并同步 DDL"}</button></div></div></div>;
}

function SettingsModal({ onClose, onToast, onSaved }: { onClose: () => void; onToast: (v: string) => void; onSaved: () => Promise<void> }) {
  const [settings, setSettings] = useState<MailSettings>(() => { try { return JSON.parse(localStorage.getItem("mail-settings") ?? "null") || defaultSettings; } catch { return defaultSettings; } });
  const [password, setPassword] = useState(""); const [testing, setTesting] = useState(false); const [saving, setSaving] = useState(false); const [result, setResult] = useState("");
  const field = (key: keyof MailSettings, value: string) => setSettings((s) => ({ ...s, [key]: typeof s[key] === "number" ? Number(value) : value }));
  return <div className="overlay" onMouseDown={onClose}><div className="modal settings-modal" onMouseDown={(e) => e.stopPropagation()}><button className="close" onClick={onClose}><X/></button><div className="modal-kicker">个人账号 · 无需管理员权限</div><h2>连接 BNBU 学生邮箱</h2><p className="muted">填写完整学生邮箱和邮箱密码；如邮箱提供客户端专用密码，请优先使用。凭据只保存在 macOS 钥匙串。</p><div className="form-grid"><label className="wide">学生邮箱<input value={settings.email} onChange={(e) => field("email", e.target.value)} placeholder="学号@mail.bnbu.edu.cn"/></label><label>IMAP 服务器<input value={settings.host} onChange={(e) => field("host", e.target.value)}/></label><label>端口<input type="number" value={settings.port} onChange={(e) => field("port", e.target.value)}/></label><label className="wide">邮箱密码 / 客户端专用密码<input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="不会写入数据库"/></label><div><strong>同步范围</strong><p className="muted">从本次启用之后的新邮件开始，不读取历史邮件</p></div><label>同步间隔（分钟）<input type="number" min="1" value={settings.syncMinutes} onChange={(e) => field("syncMinutes", e.target.value)}/></label></div>{result && <div className="connection-result">{result}</div>}<div className="modal-actions"><button className="secondary" disabled={testing || saving} onClick={async () => { setTesting(true); try { setResult(await api.testConnection(settings, password)); } catch(e) { setResult(String(e)); } finally { setTesting(false); } }}>{testing ? "测试中…" : "测试连接"}</button><button className="primary" disabled={!settings.email || !password || saving} onClick={async () => { setSaving(true); setResult("正在保存到 macOS 钥匙串并建立新邮件起点…"); try { await api.saveSettings({...settings, initialDays: 0}, password); localStorage.setItem("mail-settings", JSON.stringify({...settings, initialDays: 0})); onToast("已建立同步起点，等待新邮件"); onClose(); await onSaved(); } catch (e) { setResult(`保存失败：${String(e)}`); } finally { setSaving(false); } }}>{saving ? "正在保存…" : "保存并从现在开始"}</button></div></div></div>;
}

function ReminderModal({ draft: initial, onClose, onCreated }: { draft: ReminderDraft; onClose: () => void; onCreated: (id: string) => void }) {
  const [draft, setDraft] = useState(initial); const [saving, setSaving] = useState(false); const set = (key: keyof ReminderDraft, value: string | null) => setDraft((d) => ({...d, [key]: value}));
  return <div className="overlay" onMouseDown={onClose}><div className="modal reminder-modal" onMouseDown={(e) => e.stopPropagation()}><button className="close" onClick={onClose}><X/></button><div className="reminder-icon"><Bell/></div><div className="modal-kicker">提醒草稿</div><h2>确认重要事项</h2><p className="muted">内容由本地规则提取。创建前可自由修改。</p><label>标题<input value={draft.title} onChange={(e) => set("title", e.target.value)}/></label><label>截止时间<input type="datetime-local" value={draft.dueAt ? toLocalInput(draft.dueAt) : ""} onChange={(e) => set("dueAt", e.target.value ? new Date(e.target.value).toISOString() : null)}/></label><label>优先级<select value={draft.priority} onChange={(e) => set("priority", e.target.value)}><option value="low">低</option><option value="normal">普通</option><option value="high">高</option></select></label><label>备注<textarea rows={7} value={draft.notes} onChange={(e) => set("notes", e.target.value)}/></label><div className="modal-actions"><button className="secondary" onClick={onClose}>取消</button><button className="primary" disabled={!draft.title || saving} onClick={async () => { setSaving(true); try { await api.createReminder(draft); onCreated(draft.sourceMailId); } finally { setSaving(false); } }}>{saving ? "正在创建…" : "创建系统提醒"}</button></div></div></div>;
}

function toLocalInput(value: string) { const d = new Date(value); const off = d.getTimezoneOffset(); return new Date(d.getTime() - off * 60000).toISOString().slice(0, 16); }

function RulesModal({ onClose, onToast }: { onClose: () => void; onToast: (v: string) => void }) {
  const [rules, setRules] = useState<ClassificationRule[]>([]);
  const [draft, setDraft] = useState<ClassificationRule>({ id: "", category: "待办", field: "sender", operator: "contains", value: "", priority: 50, enabled: true });
  useEffect(() => { api.listRules().then(setRules); }, []);
  async function add() { if (!draft.value.trim()) return; const rule = {...draft, id: draft.id || crypto.randomUUID()}; await api.saveRule(rule); setRules((old) => [...old.filter((r) => r.id !== rule.id), rule].sort((a,b) => b.priority-a.priority)); setDraft({...draft,id:"",value:""}); onToast("分类规则已保存"); }
  return <div className="overlay" onMouseDown={onClose}><div className="modal rules-modal" onMouseDown={(e) => e.stopPropagation()}><button className="close" onClick={onClose}><X/></button><div className="modal-kicker">本地规则</div><h2>分类规则</h2><p className="muted">数值越高越优先；规则命中后不会再执行内置关键词分类。</p><div className="rule-builder"><select value={draft.field} onChange={(e) => setDraft({...draft,field:e.target.value as ClassificationRule["field"]})}><option value="sender">发件人</option><option value="domain">发件域名</option><option value="subject">主题</option><option value="body">正文</option></select><select value={draft.operator} onChange={(e) => setDraft({...draft,operator:e.target.value as ClassificationRule["operator"]})}><option value="contains">包含</option><option value="equals">等于</option><option value="regex">正则匹配</option></select><input aria-label="规则匹配值" value={draft.value} placeholder="匹配值" onChange={(e) => setDraft({...draft,value:e.target.value})}/><select value={draft.category} onChange={(e) => setDraft({...draft,category:e.target.value as Category})}>{categories.map((c)=><option key={c}>{c}</option>)}</select><input aria-label="规则优先级" type="number" value={draft.priority} onChange={(e) => setDraft({...draft,priority:Number(e.target.value)})}/><button className="primary" onClick={add}>添加规则</button></div><div className="rule-list">{rules.length === 0 ? <div className="rule-empty">还没有自定义规则</div> : rules.map((rule) => <div className="rule-row" key={rule.id}><span className={`tag ${colors[rule.category]}`}>{rule.category}</span><strong>{rule.field}</strong><span>{rule.operator}</span><code>{rule.value}</code><em>优先级 {rule.priority}</em><button aria-label="删除规则" onClick={async()=>{await api.deleteRule(rule.id);setRules((old)=>old.filter((r)=>r.id!==rule.id));}}><Trash2 size={14}/></button></div>)}</div><div className="modal-actions"><button className="primary" onClick={onClose}>完成</button></div></div></div>;
}
