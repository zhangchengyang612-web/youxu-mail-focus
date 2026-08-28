# 邮序 · BNBU 学生邮件智能分类与提醒

本地优先的 Tauri 2 桌面应用：学生使用自己的 BNBU 邮箱账号通过 IMAP 登录，无需管理员权限；邮件在本机分类，并可把重要内容转换为系统提醒。

## 已实现

- BNBU 学生邮箱 IMAP SSL 连接测试、从启用时刻开始的 UID 增量同步、附件跳过、UID 去重与网络超时提示。
- 通过个人 Moodle iCalendar URL 同步 BNBU iSpace Assignment、Quiz、Turnitin 等截止时间，并在独立 DDL 分区展示。
- MIME 正文提取、HTML 转纯文本、SQLite 持久化和系统钥匙串凭据存储。
- 待办、学业、校园事务、社团活动、个人、外部六类关键词分类；支持发件人、域名、主题和正文自定义规则。
- 邮件搜索、分类筛选、手动改类、批量选择、提醒草稿提取、截止时间编辑及重复导入保护。
- macOS 写入 Apple 提醒事项；Windows Microsoft To Do 适配器接口和 Entra 配置入口已预留。
- 浏览器开发模式自动使用示例邮件，不读取真实邮箱，方便界面开发。

## 开发运行

要求 Node.js 20+、Rust stable，以及对应平台的 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
npm install
npm test
npm run tauri dev
```

仅预览前端：

```bash
npm run dev
```

生成安装包：

```bash
npm run tauri build
```

macOS 生成 `.dmg`，Windows 生成 `.msi`。未签名安装包仅适合内部测试。

当前可分享的 Apple Silicon 测试包位于 `release/邮序_0.2.0_BNBU_macOS_AppleSilicon.dmg`。该版本已完成完整 ad-hoc 签名与磁盘映像校验，但未使用 Apple Developer ID 公证；其他电脑首次运行时可能需要右键应用并选择“打开”。

## BNBU 学生邮箱配置

1. 通过校园门户进入学生邮箱。
2. 在网页版邮箱“设置 → 客户端设置”中启用客户端收信；如需历史邮件，将接收范围改为全部邮件。
3. 在邮序中填写完整学生邮箱地址和邮箱密码；若提供客户端专用密码，则优先使用专用密码。
4. 点击“测试连接”，成功后点击“保存并开始同步”。不需要企业管理员认证。

默认服务器为 `imap.exmail.qq.com`、SSL 端口 `993`。密码通过操作系统安全存储保存，不进入 SQLite 或日志。

## 平台提醒

- macOS：首次创建提醒时会触发系统自动化/提醒事项权限提示。拒绝后可在“系统设置 → 隐私与安全性”中重新授权。
- Windows：发布前需注册 Microsoft Entra 应用，使用 OAuth PKCE 获取 `Tasks.ReadWrite` 权限；将 Client ID 写入构建环境。当前界面会在未配置时给出明确错误，不会静默丢失草稿。

## 数据位置与隐私

数据库位于 Tauri 应用数据目录下的 `mail-focus.sqlite`。邮件正文只在本机用于分类和提醒提取，不调用云端 AI，不加载 HTML 邮件中的远程图片，也不下载附件。可通过应用设置清除本地数据。
