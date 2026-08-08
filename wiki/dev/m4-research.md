---
title: M4 生产化技术调研
tags: [调研, M4, 打包, CI, 去重, 敏感过滤]
created: 2026-08-08
updated: 2026-08-08
status: ready
---

# M4 生产化技术调研

> 四块并行调研：本地打包 / GitHub Actions CI / 去重策略 / 敏感过滤。
> 调研日期 2026-08-08，所有结论基于官方文档 + 源码确认。

## 一、本地打包（✅ 已验证）

### 1.1 命令与产物

- **命令**：`npm run tauri build`（Tauri 2）
- **产物位置**：`src-tauri/target/release/bundle/`
- **macOS 产物**：
  - `macos/Clipboard Manager.app` — Application Bundle（13M）
  - `dmg/Clipboard Manager_1.0.0_aarch64.dmg` — 磁盘映像（4.8M）
- **实测**：2026-08-08 本地一次性打包成功（1m25s 编译），零错误。

### 1.2 当前 tauri.conf.json 配置

- `bundle.targets = "all"`：在每个平台只构建该平台格式，不会跨平台（macOS 上只出 app+dmg）。
- `bundle.macOS.minimumSystemVersion = "11.0"`：已设。
- `bundle.icon`：icns(mac) + ico(win) + png 齐全。
- **缺 `bundle.windows` 块**：做 Windows 签名/NSIS 定制时需补。

### 1.3 bundle.targets 说明

- `"all"`（当前值）= 各平台构建自身支持的格式
- 可用数组指定子集：`["app", "dmg"]`（mac）/ `["msi", "nsis"]`（win）
- 也可 CLI `--bundles` 临时指定

## 二、GitHub Actions CI 跨平台打包

### 2.1 官方方案：tauri-apps/tauri-action

- **Action**：[tauri-apps/tauri-action@v1](https://github.com/tauri-apps/tauri-action)
- 明确支持 Tauri 2，无需换 Action（v1 和 v2 共用）。
- 核心能力：构建 → 上传 GitHub Release（`tagName` / `releaseName` / `releaseDraft` 等参数）。

### 2.2 macOS runner 现状（关键）

- **`macos-latest` 现在是 ARM（Apple Silicon）**，2024-02 从 Intel 切换，2025-08 迁到 macOS 15（仍 ARM）。
- macOS 13（最后 Intel `macos-latest`）2025-12-04 退役。
- **双架构打包方案**：一个 ARM runner 即可：
  - `aarch64-apple-darwin` — 原生编译（最快）
  - `x86_64-apple-darwin` — 交叉编译（需 rust-toolchain 装两个 target）
- 不需要 Intel runner。

### 2.3 matrix 配置（本项目推荐）

```yaml
matrix:
  include:
    - platform: 'macos-latest'
      args: '--target aarch64-apple-darwin'   # Apple Silicon
    - platform: 'macos-latest'
      args: '--target x86_64-apple-darwin'     # Intel（交叉编译）
    - platform: 'windows-latest'
      args: ''                                 # .msi + .exe
```

### 2.4 Windows 产物

- `.msi`（WiX 安装包）+ `.exe`（NSIS 安装包），Tauri 2 默认同时产出。
- runner：`windows-latest`（Windows Server 2022），无需额外系统依赖。

### 2.5 触发方式

两种均可（官方默认是 release 分支触发）：
- **推 release 分支**：`on: push: branches: [release]`
- **push tag**：`on: push: tags: ['v*']`（更灵活，推荐）

必须加 `permissions: contents: write`，否则无权创建 release。

### 2.6 代码签名与公证

**结论：M4 首期不签名（无证书），先跑通 CI 打包流程。后续按需补签名。**

| 平台 | 无签名体验 | 需要什么 |
|------|-----------|---------|
| macOS | Gatekeeper 拦截，用户需「系统设置→隐私与安全性」手动放行 | Apple Developer ID 证书 + notarization |
| Windows | SmartScreen 「Windows 已保护你的电脑」警告 | OV/EV 代码签名证书 |

- macOS 公证方式：App Store Connect API key（推荐）或 Apple ID + 密码 + Team ID。
- Windows：OV 证书仍弹警告（需积累信誉），EV 证书立即无警告（最贵）。
- **updater 签名是另一套**：`TAURI_SIGNING_PRIVATE_KEY`（`tauri signer generate`），与安装包签名无关。M4 暂不做自动更新。

### 2.7 关键依赖缓存

- `swatinem/rust-cache@v2`：Rust 编译缓存（CI 第二次构建快很多）
- `actions/setup-node@v4` + `cache: 'npm'`：npm 缓存

## 三、去重策略

### 3.1 主流方案（Maccy 源码确认）

**Maccy 去重行为（`HistoryItem.supersedes` + `History.add`）**：
1. **全局去重**（遍历整个历史，不限最近 N 条）
2. 命中后：保留旧 item 的 `firstCopiedAt` + `pin`（置顶），累加 `numberOfCopies`
3. **删旧插新**（Core Data delete + insert）→ 效果是移到最前
4. 比较前过滤 `transientTypes`（噪音格式），再 `allSatisfy` 全格式匹配

Raycast / Ditto / Alfred 同理：全局去重 + 移到最前。

### 3.2 本项目现状评估

**已有的（正确）**：
- ✅ 全局去重（`items.rs` 的 `SELECT item_id FROM clipboard_formats WHERE uti=? AND data_blob=?`）
- ✅ 命中后 UPDATE `last_copied_at` + `copy_count+1`
- ✅ 列表 `ORDER BY last_copied_at DESC` → UPDATE 后自动移到最前
- ✅ 图片去重（sha256 文件名分桶 + `DedupKey::Image(file_path)`）
- ✅ 保留 `first_copied_at` 不变

**发现的缺口（3 个）**：

| 缺口 | 描述 | 优先级 |
|------|------|--------|
| **G1 富文本格式残留** | `insert_or_update_multi` 命中重复时只 UPDATE items，不更新 formats。A(带RTF)→B→A(纯文本) 时旧 RTF 残留 | P0 |
| **G2 文件 URL 不去重** | `DedupKey` 没有处理 `public.file-url`，文件 URL 不参与去重 | P1 |
| **G3 hash 索引** | 当前 `data_blob = ?` 全表扫。万条内无感，超 5000 条可能需 `content_hash` 列 + 索引 | P2（暂不做） |

### 3.3 修复方案

- **G1**：`insert_or_update_multi` 命中后，DELETE 旧 formats + INSERT 新 formats（同事务）
- **G2**：`DedupKey` 加 `File(&str)` 分支，用 URL 字符串去重
- **G3**：暂不做。若将来做，复用 `sha2::Sha256`（已有依赖），加 `content_hash TEXT` 列 + 索引

### 3.4 hash 选型（如果将来做）

| 方案 | 评价 |
|------|------|
| `std::hash::DefaultHasher` | ❌ 随机种子，重启后 hash 变化，不可存 DB |
| `sha2::Sha256` | ✅ 推荐（已依赖，确定性，零碰撞风险） |
| `xxhash-rust` (XXH3) | ✅ 最快，但要加新依赖 |

## 四、敏感信息过滤

### 4.1 业界共识（关键发现）

**没有任何主流剪贴板管理器做"纯文本密码启发式识别"。** 都靠三层防御：

| 层 | Maccy | Raycast | 本项目 |
|----|-------|---------|--------|
| 第 1 层：nspasteboard 标记 | ✅ Concealed/Transient 跳过 | ✅ 同 | ✅ M2 已有 |
| 第 2 层：应用黑名单 | ✅ ignoredApps（bundle id） | ✅ Disabled Apps | ❌ 缺 |
| 第 3 层：正则黑名单 | ✅ ignoreRegexp | ❌ | ❌ 缺 |

原因：纯文本启发式（高熵检测）误报率太高（见 4.3）。

### 4.2 没有拿来即用的 Rust crate

| crate | 评价 |
|-------|------|
| `ripsecrets` | ⭐ 最值得参考，但是 CLI 非库，未发布 crates.io。抄思路 |
| `secretscan` | 质量不明，下载量低，不建议依赖 |
| `gitleaks` | Go 写的，业界标准。`gitleaks.toml` 是最权威的正则模式来源 |

**结论：从 gitleaks 抄正则模式，自己写 ~50 行检测函数。**

### 4.3 高熵检测（Shannon entropy）—— 不推荐

- 误报率太高：UUID / Base64 图片 / git commit hash / 中文随机串全误判。
- gitleaks 8.20→8.24 因熵检测误报引发社区投诉。
- 业界正用 BPE token efficiency 替代 Shannon 熵。
- **ripsecrets 的改进**：用概率论三指标（distinct values / 字符类别分布 / bigram 频率），比裸熵好但复杂（~150 行），仍只是辅助。
- **结论：不做高熵检测。**

### 4.4 推荐的敏感模式（从 gitleaks 核实）

高置信度（固定前缀，几乎零误报）：
```
AWS Access Key       AKIA[0-9A-Z]{16}
GitHub PAT           gh[pousr]_[0-9a-zA-Z]{36}
GitLab PAT           glpat-[0-9a-zA-Z\-]{20}
Slack Token          xox[baprs]-[0-9a-zA-Z\-]{10,48}
Stripe Key           sk_live_[0-9a-zA-Z]{24}
Google API           AIza[0-9A-Za-z\-_]{35}
RSA Private Key      -----BEGIN (?:RSA |EC |OPENSSH |PGP )?PRIVATE KEY-----
```

### 4.5 浏览器扩展盲区（已知，无法根治）

- 1Password/Bitwarden **浏览器扩展**复制密码只写纯文本，无任何标记。
- 任何剪贴板管理器都无法 100% 防住——这是浏览器扩展的 bug。
- 应对：README 告知 + 推荐用密码管理器桌面 app + 应用黑名单。

### 4.6 实施方案

| 层 | 实现 | 优先级 |
|----|------|--------|
| 第 1 层 | ✅ M2 已有（`PRIVACY_SKIP_UTIS`） | 已完成 |
| 第 2 层 正则黑名单 | `is_sensitive(text) -> bool` 纯函数 + gitleaks 模式 | **P0** |
| 第 3 层 应用黑名单 | 设置面板加排除应用（需 clipboard-rs 暴露 source app） | **P1**（可能受 clipboard-rs 限制） |

## 五、调研来源

### 打包/CI
- [Tauri GitHub Actions 官方指南](https://v2.tauri.app/distribute/pipelines/github/)
- [tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action)
- [macOS Code Signing](https://v2.tauri.app/distribute/sign/macos/)
- [Windows Code Signing](https://v2.tauri.app/distribute/sign/windows/)
- [Environment Variables](https://v2.tauri.app/reference/environment-variables/)

### 去重
- [Maccy HistoryItem.supersedes 源码](https://github.com/p0deje/Maccy/blob/master/Maccy/Models/HistoryItem.swift)
- [Maccy History.add 全局去重+移到最前](https://github.com/p0deje/Maccy/blob/master/Maccy/Observables/History.swift)
- [Raycast 剪贴板手册](https://manual.raycast.com/clipboard-history)
- [Maccy issue #1124 去重行为讨论](https://github.com/p0deje/Maccy/issues/1124)

### 敏感过滤
- [gitleaks 默认规则 gitleaks.toml](https://github.com/gitleaks/gitleaks/blob/master/config/gitleaks.toml)
- [ripsecrets 源码](https://github.com/sirwart/ripsecrets)
- [GitHub secret 扫描模式参考](https://docs.github.com/en/code-security/secret-scanning/using-the-secret-scanning-and-code-scanning-api/secret-scanning-pattern-reference)
- [PMC：Shannon 熵误报率研究](https://pmc.ncbi.nlm.nih.gov/articles/PMC10934942/)
- [Bitwarden 浏览器扩展盲区 #326](https://github.com/bitwarden/browser/issues/326)
