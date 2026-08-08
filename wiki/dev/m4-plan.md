---
title: M4 生产化规划
tags: [规划, M4, 打包, CI, 去重, 敏感过滤]
created: 2026-08-08
updated: 2026-08-08
status: draft
---

# M4 生产化规划

> 调研依据见 [[m4-research]]。用户决策：打包优先。
> 启动日期 2026-08-08。

## 目标

将 Clipboard Manager 从「能跑的 dev 版」变成「能交付的安装包」：
1. CI 自动跨平台打包（macOS .dmg + Windows .msi/.exe）
2. 去重补全（3 个缺口）
3. 敏感信息过滤（正则黑名单）
4. Windows 平台验证（CI 产物在 PD 虚拟机实测）

## 阶段划分

### 阶段 A：GitHub Actions CI 跨平台打包（P0，先做）

**目标**：push tag → CI 自动构建 macOS(arm64+x86_64) + Windows → 上传 GitHub Release。

**交付物**：
- `.github/workflows/release.yml` — CI workflow
- 三平台 matrix（mac arm64 / mac x86_64 / windows）
- tag 触发 + 草稿 release
- Rust/npm 缓存加速

**不做**：
- 代码签名/公证（无证书，后续补。README 告知用户手动放行）
- 自动更新（updater plugin，远期）

**实现步骤**：
1. 写 `release.yml`（基于官方示例，适配本项目 src-tauri 路径）
2. 首次 push tag 测试 CI 是否跑通
3. 验证三个平台产物都上传到 Release

**验收标准**：
- [ ] push `v*` tag → CI 触发
- [ ] macOS arm64 .dmg 产出
- [ ] macOS x86_64 .dmg 产出
- [ ] Windows .msi + .exe 产出
- [ ] 全部上传到 GitHub Release（草稿）

### 阶段 B：去重缺口修复（P0）

**目标**：补全 M2 遗留的 3 个去重缺口。

| 缺口 | 修复 | 优先级 |
|------|------|--------|
| G1 富文本格式残留 | 命中重复时 DELETE 旧 formats + INSERT 新 formats（同事务） | P0 |
| G2 文件 URL 不去重 | DedupKey 加 `File(&str)` 分支 | P1 |
| G3 hash 索引 | 暂不做（万条内无感） | P2（不做） |

**随写随测**：每个修复同步补单测。

**验收标准**：
- [ ] G1：A(带RTF)→B→A(纯文本) 后，回填的 A 是纯文本（旧 RTF 不残留）
- [ ] G2：同一文件 URL 复制两次，列表只有一条
- [ ] cargo test 全绿

### 阶段 C：敏感信息过滤（P0）

**目标**：识别 API key/token/私钥等已知模式，自动拦截不入库。

**实现**：
1. `clipboard/sensitive.rs`：`is_sensitive(text: &str) -> bool` 纯函数
2. 内置 gitleaks 正则模式（AWS/GitHub/GitLab/Slack/Stripe/Google/私钥）
3. handler.rs 捕获时调用，命中则跳过 + log
4. 设置面板加开关（默认开）
5. ⚠️ 依赖决策：用 `regex` crate（成熟）vs 手写前缀匹配（零依赖）
   - 倾向 `regex`（模式复杂，手写易错）
   - 但需评估编译影响

**不做**：
- ❌ 高熵检测（误报率不可接受）
- ❌ 应用黑名单（P1，可能受 clipboard-rs 限制，需先确认能否拿 source app）

**随写随测**：`is_sensitive` 纯函数补充足量单测（正例 + 反例 + 边界）。

**验收标准**：
- [ ] 复制 `ghp_xxxx...` → 不入库
- [ ] 复制 `-----BEGIN RSA PRIVATE KEY-----` → 不入库
- [ ] 复制普通文本 → 正常入库
- [ ] 设置面板可开关
- [ ] cargo test 全绿

### 阶段 D：Windows 平台验证（P1）

**目标**：确认应用在 Windows 上能正常工作。

**前置条件**：阶段 A CI 打出 Windows 包。

**验证项**（在 PD 虚拟机里）：
1. 安装包能安装运行
2. 剪贴板监听正常（clipboard-rs Windows 路径）
3. 全局快捷键正常
4. 托盘图标正常
5. 开机自启正常（Windows 用注册表，非 LaunchAgent）
6. 设置持久化正常

**已知风险**：
- `clipboard-rs` 声称跨平台，但 Windows 的 `AddClipboardFormatListener` 路径未实测
- `tauri-plugin-autostart` 在 Windows 上用注册表（应该能工作）
- 自启参数 `--hidden` 在 Windows 上的行为

**验收标准**：
- [ ] Windows 安装+启动成功
- [ ] 核心功能（监听/搜索/回填）正常
- [ ] 记录所有异常并决定是否修复

### 阶段 E：收尾 + PR review

**步骤**：
0. `zcode-pr-review` Skill 跑深度 review（session-checklist 步骤 0 强制）
1. 更新 progress.md + TODO.md
2. README 加安装说明（未签名包的用户放行指引）
3. commit

## 不做（明确排除）

- ❌ 代码签名/公证（无证书，后续单独做）
- ❌ 自动更新（updater plugin，远期）
- ❌ 高熵检测（误报率不可接受）
- ❌ 应用黑名单（P1，留 M4.5 或后续）
- ❌ Linux 打包（当前只 macOS + Windows）
- ❌ content_hash 索引优化（万条内无感）

## 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| CI 首次跑可能有环境问题 | 阶段 A 延迟 | 先用 `workflow_dispatch` 手动触发调试 |
| clipboard-rs Windows 路径有 bug | 阶段 D 失败 | 如果不行，Windows 监听可能需换方案（rdev/Win32 FFI） |
| 正则 crate 增加编译时间 | 阶段 C | 评估编译影响，必要时手写前缀匹配 |
