---
title: 开发进度（状态唯一真相源）
tags: [进度, 状态]
created: 2026-06-29
updated: 2026-08-09
status: active
---
---

# 开发进度

> **📌 当前状态（唯一真相源）**：版本号 / 测试数 / lint 只在本段维护，别处放指针。

## 当前状态

- **项目**：Clipboard Manager（轻量常驻的剪贴板历史管理桌面应用）
- **出身**：2026-08-05 从模板 `tauri-sidecar-template` 复制创建（commit `ac02fed`）
- **当前里程碑**：UI 增强迭代 — **✅ 完成**(2026-08-09,多主题+透明度+存储统计+内容识别+类型筛选+双击回填)
- **架构现状**：纯 Tauri 2 单进程(Rust 既是壳也是后端),无 sidecar / 无 HTTP
  + M3 新增:全局快捷键 / 托盘 / 开机自启 / 设置持久化(Store 插件)/ 自动清理
  + M4 新增:敏感信息过滤(正则黑名单)+ 去重修复(格式覆盖/文件 URL)+ CI 跨平台打包
  + M4 UI 新增:深色毛玻璃主题(lucide-react 图标库 + 苹果配色 token + 三组件重做)
  + 多主题切换新增:浅色(.theme-light)+ 折中(.theme-mid)+ 分段按钮选择器 + localStorage 防闪
  + UI 增强新增:窗口透明度可调 + 存储统计卡片 + 清除历史 + 内容识别(link/markdown)
    + 类型筛选 tag + 类型图标 + 双击回填 + 图片/文件路径展示
  + Schema v3:content_subtype 列(写入时识别 link/markdown 子类型)
- **技术栈**：Tauri 2(Rust) + React 18 + Tailwind + shadcn/ui + zustand
  + clipboard-rs 0.3(剪贴板读写/监听/多格式)+ rusqlite 0.31(存储,bundled,**已含 FTS5**)
  + sha2 0.10(图片去重文件名)+ image dev-dep(测试生成 PNG)
  + tauri-plugin-global-shortcut 2 / tauri-plugin-autostart 2 / tauri-plugin-store 2(M3 新增)
  + regex 1(M4 新增,敏感信息正则匹配)
  + lucide-react 1.30(M4 UI 新增,矢量图标库)
- **验证基线**：cargo test 209 passed + npm test 67 passed + npm build 全绿 + clippy 零 warning
- **下一步**：M4 阶段 D(Windows 实测 + CI 打包发布)— 需先建 GitHub 仓库 + push tag v1.0.0

## M1 调研 + Spike 记录(2026-08-05)

- **技术调研**(见 [[m1-research]]):三块并行调研
  - 监听机制:macOS 无原生通知,只能轮询 changeCount(500ms 共识值)
  - crate 选型:**clipboard-rs**(用户决策:功能全 + 后期可 fork;放弃 arboard 因能力受限)
  - 多格式存储:items + formats 一对多两表(照搬 Maccy/Ditto),图片混合存储(Raycast 方案)
- **Spike 验证**(见 [[m1-research]] 第四节):
  - 🔴 核心验证:**clipboard-rs watcher 在 Tauri 后台线程不触发 NSPasteboard 崩溃**(5 次实测通过)
  - → M1 监听架构定:后台线程 + clipboard-rs watcher,无需 dispatch 主线程
  - 实测发现:`get_text()` 无文本格式时报错(文件场景),正式实现需 `available_formats()` 容错
- **M1 规划落盘**(见 [[m1-plan]]):5 阶段(存储/监听/命令/UI/实测)
- **Spike 代码**:`spike_clipboard.rs` + `SpikeClipboardProbe.tsx` 临时验证,阶段 B 演化删除

## M1 实施完成记录(2026-08-06)

5 阶段施工,全链路打通:复制文字 → 监听捕获 → 存 DB → 列表展示 → 上下键 → 回车回填。

- **阶段 A**(`1c41df7`):rusqlite 存储层(items/formats 两表 + CRUD + 15 单测)
  - 决策调整(动态校准):migration 用手写 `CREATE TABLE IF NOT EXISTS`,不引 refinery(过重)
  - 踩坑:`execute_batch` 的反斜杠续行让 SQL `--` 注释吞掉建表语句,改 raw string 修复
- **阶段 B**(`08e230b`):剪贴板监听层(clipboard-rs watcher 后台线程 → DB → emit 前端)
  - 模块化:`clipboard/mod.rs`(生命周期)+ `handler.rs`(捕获逻辑),平台无关
  - watcher 线程内开独立 DB 连接(Connection 非 Send;WAL 模式并发安全)
  - 删 spike 临时代码,演化成正式模块
- **随写随测纪律落地**(`63ac4ea`):用户强调最高优先级工程纪律,补 handler 7 单测 + 写进核心原则 8 / §1.22
- **阶段 C**(`2c40f58`):command 接口(list_items / copy_back / delete_item)+ 错误码两端同步
  - 两层结构:核心逻辑函数(接 &Connection,可测)+ command 包装(接 State)
- **阶段 D**(`edbe2b0`):前端历史列表 UI(types/hook/ClipboardList/SpotlightPanel)
  - 键盘交互:↑↓ 循环导航 / ⏎ 回填 / ⌫ 删除
  - 踩坑:vite/tsconfig/vitest 三处别名要同步加 @hooks(vitest 有独立 config 不读 vite)
- **阶段 E**:端到端实测 6 项全通过(复制实时更新 / 导航循环 / 回车回填 / 去重 ×N / 删除 / 中文代码)

**M1 期间确立的两条核心工程纪律**(用户强调,已进跨会话必读):
- 核心原则 7 / §1.21:模块化·高内聚低耦合(大模型开发复杂度控制根基)
- 核心原则 8 / §1.22:随写随测(写完+测完+通过才算闭环)

**M1 遗留 → M3 决策**(用户确认):
- 窗口常驻问题(visible:true)→ M3 做全局快捷键唤起/隐藏
- 交互方案:唤起 + 回车回填(主流方案,Maccy/Flycut 同款)
- ⚠️ 「松手回填」做不到:Tauri 全局快捷键只支持 keydown 不支持 keyup([文档](https://v2.tauri.app/reference/javascript/global-shortcut/)),要松手检测需 OS hook(rdev/Carbon)+ 触发辅助功能权限,工程量大,M3 不做

**M1 收尾 PR review**(用 `zcode-pr-review` Skill,两批 4 agent 并行,commit `995d0f3` + `726a9fe`):
- 🔴 **Critical(置信92)已修**:copy_back 回填触发监听反馈循环(copy_count 虚高 + 列表顺序错乱)。修复:共享 `Arc<AtomicBool>` 标志位,copy_back 写剪贴板前置位,handler `should_skip_by_flag` 消费跳过。实测验证:回填后 copy_count 不再虚高。
- 🟡 **Important 已修**:前端 handleCopyBack/handleDelete 错误反馈缺失(违反 §8)→ 加 actionError 状态,底栏显示失败提示(3 秒清除)
- 🟡 **第二批修复**:TS 加 Zod 校验(§8 边界校验)+ ClipboardItem readonly + self_write 测试质量(抽 should_skip_by_flag 纯函数测业务契约)+ from_db_str 未知值加 log + 注释/文档清理 + listen Promise rejection 处理
- **附带修**:commands 测试时序竞态(同毫秒插入导致 DESC 排序不确定)
- **不修(过度设计/刻意权衡)**:AppError.code 改枚举(前端字符串比对设计)、AppState.db() 改 Result、UseClipboardItemsResult 改 discriminated union
- **价值印证**:review 发现了置信 92 的 Critical bug(单靠自检清单发现不了),已据此把 `zcode-pr-review` 写进 code-review-checklist + session-checklist 步骤 0 强制使用

## M2 调研 + Spike 记录(2026-08-06)

- **技术调研**(见 [[m2-research]]):两块并行调研
  - FTS5 中文搜索:🔴 决定性发现 —— **`bundled` feature 已无条件编译 FTS5**(libsqlite3-sys build.rs),
    Cargo.toml 零改动。P0 = unicode61 + 应用层 bigram 预处理 + 单字 LIKE 兜底;P1 留 jieba。
  - 图片存储 + 多格式:混合方案(文本进 BLOB,图片进文件系统 sha256 分桶 + 引用计数);
    白名单(照搬 Maccy)+ 隐私黑名单(`ConcealedType`/`TransientType`);
    ⚠️ 密码管理器**浏览器扩展**是盲区(只防住桌面 app,README 告知)。
- **Spike 验证**(FTS5 + bigram,8 测试全绿):bundled 含 FTS5 实测确认 / bigram 中文召回 / 英文代码 /
  BM25 排序 / LIKE 兜底 / rebuild / 触发器同步 全通过。spike 代码 `fts_spike.rs` 待 M2 施工演化后删。
- **M2 规划落盘**(见 [[m2-plan]]):5 阶段(A schema/migration → B 多格式捕获回填 → C 图片 → D 搜索 → E 实测)。
  用户决策:migration 手写版本号 / 施工顺序多格式→搜索→图片 / 范围全做。

## M2 实施完成记录(2026-08-07)

5 阶段施工,搜索 + 富文本/图片全链路打通,GUI 实测 + PR review 全过。

- **阶段 A**(`2bd6f96`):schema migration(手写 `PRAGMA user_version`)+ FTS5 外部内容表 + 三触发器 + 多格式存储基础
  - 🔴 决定性发现:**bundled feature 已含 FTS5**(libsqlite3-sys build.rs 无条件编译),Cargo.toml 零改动
  - 踩坑:migration 回填必须在建 FTS 触发器之前(否则 UPDATE 触发 au 对不存在的 FTS 行发 delete → malformed)
- **阶段 B**(`a491350` + `fa8960c`):多格式捕获(白名单照搬 Maccy + 隐私黑名单)+ 多格式回填(set 原子写)
  - 新模块:uti.rs(白/黑名单)/ capture.rs(classify_formats 纯函数)/ title.rs(按 kind 生成标题)
  - 实测:富文本回填保留格式(加粗/链接)/ 文件 URL 回填粘贴出文件
  - fix:serde 缺 rename_all=camelCase(字段名不匹配)/ contentKind schema 扩联合类型 / 列表滚动
- **阶段 C**(`586d415` + `4104893`):图片存储(sha256 分桶 + to_png)+ 缩略图(asset protocol)+ 引用计数删除 + 图片去重
  - 新模块:image_store.rs(save_image / count_references / delete_if_last_reference)
  - 踩坑:缩略图相对/绝对路径不一致(emit 相对、command 绝对 → 抽 resolve_thumb_absolute 统一)
  - 踩坑:纯图片无文本去重缺失(语音工具反复写同一图 → 一张变 4 份)→ 加 DedupKey::Image 按 file_path 去重
  - 实测:截图 → 缩略图显示 + 「图片 W×H」→ 回填粘贴出图片
- **阶段 D**(`f2124c3` + `cd94899`):搜索功能(FTS5 + bigram + 搜索框 UI + 选中滚动)
  - search_items command + useClipboardItems 加 search 状态机 + SpotlightPanel 搜索框
  - 踩坑:搜索导致列表无限闪烁(search 没 useCallback → debounce effect 无限重建)→ useCallback 修复
  - 体验:选中项自动滚动到可视区(scrollIntoView nearest)
- **PR review**(commit `865d8d2`,5 agent 并行):4 Critical + 5 Important 全修
  - 🔴 migration 无事务 → 包 unchecked_transaction(中途失败 ROLLBACK,避免 app 永久启动失败)
  - 🔴 搜索结果图片缩略图丢失 → search 补 fill_image_fields
  - 🔴 capture get_image 吞错 → 改 log::warn
  - 🔴 image_size 死链路(四层全断)→ 删除 ImageSize struct + 字段
  - 🟡 FTS 特殊字符注入(转义)/ PNG 解码硬错误 / 注释清理 / copy_back 错误信息 / 路径转换
  - 🧪 补图片引用计数 delete 端到端测试(2 项)
  - 不修(留 TODO):copy_back 抽纯函数 / 搜索状态机测试 / newtype 固化类型

**M2 期间确立的技术决策**(已沉淀 constraints.md):
- SQLite migration 用手写 `PRAGMA user_version`(不引 rusqlite_migration),ALTER TABLE 不幂等需版本号守护
- FTS5 中文:P0 = unicode61 + 应用层 bigram + LIKE 单字兜底(零依赖);P1 留 jieba
- 图片存储:混合方案(文本进 BLOB,图片进文件系统 sha256 分桶 + 引用计数)
- 多格式捕获:白名单(照搬 Maccy)+ 隐私黑名单(`ConcealedType`/`TransientType` 整条跳过)
- ⚠️ 密码管理器浏览器扩展是盲区(只防住桌面 app,README 告知)
- asset protocol 显示磁盘图片:`tauri.conf.json` 加 scope + Cargo.toml tauri 加 `protocol-asset` feature

## M2 后改进完成记录(2026-08-08)

PR review 留的 3 项 code quality 改进,独立改进会话清完。每项一个 commit。

- **newtype 固化类型**(`e298156`):
  - 新增 `clipboard/types.rs`:`WhitelistUti`(enum,6 个白名单穷尽安全)+ `RelPath` / `AbsPath`(serde transparent)
  - `image_store.rs` 路径函数返回 `RelPath`,`resolve_thumb_absolute` 返回 `AbsPath`
  - `models.rs` `ClipboardItem.thumb_path` 改 `Option<AbsPath>`
  - `fill_image_fields` 接收 `images_dir` 一步到位生成绝对路径(消除了 command 层重复 `resolve_thumb_absolute`)
  - `commands/clipboard.rs` `copy_back` match 改用 `WhitelistUti`(穷尽 enum,未知格式安全跳过)
  - 影响:`get_item/list/insert_or_update/insert_or_update_multi/search` 均加 `images_dir` 参数
- **copy_back 抽纯函数**(`fe8b8e9`):
  - 组装循环从 `copy_back_with_conn` 抽成 `assemble_contents(formats, images_dir) -> AssembledContents`
  - 返回 `{ contents, title, image_read_error }`,可在无真实剪贴板下单测
  - 补 8 个测试:纯文本/富文本/文件 URL(单+多)/未知格式跳过/图片文件不存在/空列表/title 优先级
- **前端搜索状态机测试**(`91d205b`):
  - 补 7 个 `renderHook` 集成测试覆盖搜索状态机分支
  - 搜索模式忽略 added 事件(searchQueryRef 闭包防过时)/ 非搜索模式接收 / search('') 回退 / search 引用稳定
  - 技术点:`vi.hoisted` 解决 vi.mock 工厂作用域;`flushAsync()` 用 setTimeout(0) flush 微任务

**验证基线**:cargo test 125 passed(原 107 + newtype 10 + assemble 8)+ npm test 27 passed(原 20 + 搜索 7)+ clippy 零 warning

## M3 调研 + 规划记录(2026-08-08)

- **技术调研**(见 [[m3-research]]):四块并行调研,全部官方文档/docs.rs 源码确认
  - 全局快捷键:`tauri-plugin-global-shortcut`(Tauri 2 register 不接 handler → with_handler 在 Builder 注册)
  - 托盘:Tauri 2 内置 `tray-icon` feature + `TrayIconBuilder`(与 v1 API 完全不同)
  - 开机自启:`tauri-plugin-autostart`(LaunchAgent,无权限弹窗)
  - 配置持久化:`tauri-plugin-store`(settings.json,与 rusqlite 职责分离)
  - 🔴 关键陷阱:hide() 触发 Focused(false) 连锁 → SUPPRESS_BLUR_HIDE AtomicBool guard
  - 🔴 进程常驻:RunEvent::ExitRequested prevent_exit(末窗口关闭不退出)
- **M3 规划落盘**(见 [[m3-plan]]):6 阶段(A 设置层 → B 快捷键+窗口 → C 托盘+自启 → D 回填增强 → E 设置面板 → F 清理+实测)
  用户决策:7 项配置(自启/快捷键/启动窗口/历史上限/保留天数/容量上限/失焦隐藏)

## M3 实施完成记录(2026-08-08)

6 阶段施工,常驻体验全链路打通,GUI 实测 + PR review 全过。

- **阶段 A**(`0e8b283`):设置持久化层
  - `settings/mod.rs`:Settings struct(6 字段 serde camelCase)+ Store 插件读写 + 7 单测
  - `commands/settings.rs`:get_settings / update_settings command
  - `lib.rs`:setup 加载 store 插件 + 读初始设置 → AppState 加 `settings: Mutex<Settings>`
  - 错误码两端镜像(SETTINGS_LOAD_FAILED / SETTINGS_UPDATE_FAILED / SHORTCUT_REGISTER_FAILED)
- **阶段 B**(`f26b41c`):全局快捷键 + 窗口控制
  - `window_control.rs`:toggle/hide_main/show_main + SUPPRESS_BLUR_HIDE guard(AtomicBool)
  - `lib.rs`:global-shortcut 插件 + with_handler(toggle) + on_window_event(失焦+close拦截) + ExitRequested prevent_exit
  - tauri.conf.json:visible:false(代码控制显示时机)
- **阶段 C**(`3bfc621`):托盘 + 开机自启
  - TrayIconBuilder + 菜单(显示/退出),左键 toggle / 右键菜单
  - tauri-plugin-autostart(LaunchAgent, --hidden),不在 setup 强制 enable
- **阶段 D**(`28f47e6`):回填增强
  - copy_back_with_conn 加 UPDATE last_copied_at(⚠️ 不加 copy_count)
  - 前端 handleCopyBack 成功后 → moveToFront + hideWindow;Esc 有搜索→清空 / 无搜索→隐藏
- **阶段 E**(`8eda6fa`):设置面板 UI
  - SettingsPanel.tsx:7 项配置(自启 Toggle / 快捷键录入器 / Select / Toggle)
  - App.tsx:view state(main ↔ settings)
  - shortcut.ts:e.code 物理键位(非 e.key 字符)避免输入法特殊字符问题
- **阶段 F**(`573cc42`):自动清理
  - cleanup.rs:cleanup_by_retention / cleanup_by_capacity / cleanup_by_count + run_cleanup(包事务)
  - handler.rs:每次成功写入后触发,联动图片引用计数删除
- **快捷键 bugfix**(`d02cbfc`):录入器改用 e.code(物理键位)避免中文输入法 √ 字符导致 Tauri 报错

**M3 期间确立的技术决策**(待沉淀 constraints.md):
- 配置持久化用 tauri-plugin-store(settings.json),与 rusqlite 职责分离
- 窗口隐藏 guard(SUPPRESS_BLUR_HIDE AtomicBool)防 hide()→Focused(false) 连锁
- 全局快捷键 handler 在 plugin Builder 的 with_handler(Tauri 2 register 不再接 handler)
- 快捷键录入用 e.code 物理键位(非 e.key 字符),避免输入法干扰
- 自动清理包 unchecked_transaction,中途失败回滚

## M3 PR review 完成记录(2026-08-08)

5 agent 并行(code-reviewer / silent-failure-hunter / type-design-analyzer / pr-test-analyzer / comment-analyzer),
2 Critical + 8 Important + 5 注释 + 3 测试缺口,全部修复(`1efb888`)。

- 🔴 **C1 已修**:update_settings 整个临界区纳入一次 Mutex 锁(防并发改快捷键交错导致快捷键永久失效)
- 🔴 **C2 已修**:App.tsx 解构 loading/error,设置加载失败显示错误态而非静默
- 🟡 **I1 已修**:前端 hideWindow 改走后端 hide_window command(统一 SUPPRESS_BLUR_HIDE guard)
- 🟡 **I2 已修**:cleanup 容量清理改累计删除字节数(避免反复全表 SUM)+ 包 unchecked_transaction
- 🟡 **I3+I4+I5 已修**:window_control hide/show/set_focus/is_visible + 快捷键回滚 全补 log(原 `let _ =` / `unwrap_or(false)` 静默吞错)
- 🟡 **I6 已修**:max_items 幽灵字段落地 — 接入 list_items limit + 新增 cleanup_by_count
- 🟡 **I7 已修**:Zod SettingsSchema 数值字段加 .int().nonnegative()
- 🟡 **I8 已修**:settings_from_value 解析失败 log::warn(原 unwrap_or_default 静默吞损坏)
- 🔵 **注释修正**:A1 #13511 引用错误 / A2 rename_all → 逐字段 rename / A4 触发措辞 / A5 invoke 参数格式
- 🔵 **测试补充**:shortcut_changed/should_run_cleanup 纯函数 7 测试 + moveToFront action 集成 3 测试 + cleanup_by_count 5 测试

**验证基线**:cargo test 159 passed(原 147 + review 补 12)+ npm test 55 passed(原 52 + review 补 3)+ clippy 零 warning

## M4 生产化实施完成记录(2026-08-09)

3 阶段施工(A+B+C),打包 CI + 去重修复 + 敏感过滤,PR review 全过。
> 用户决策:打包优先。A+B+C 本地做完后结束会话去做 UI 改造,D(Windows 实测)留 UI 完成后。

- **技术调研**(见 [[m4-research]]):四块并行调研
  - 本地打包:`npm run tauri build` 一次成功(1m25s),.dmg 4.8M
  - CI:tauri-apps/tauri-action + macos-latest(ARM 双架构)+ windows-latest
  - 去重:Maccy 源码确认(全局去重+移到最前),发现 3 缺口(G1 格式残留/G2 文件URL/G3 hash 暂不做)
  - 敏感过滤:业界无纯文本启发式(高熵误报被弃),共识三层防御,选 gitleaks 正则黑名单
- **阶段 B 去重修复**(`86f78e5`):items.rs G1+G2
  - G1:命中去重时替换 formats(DELETE旧+INSERT新,同事务),replace_formats 辅助函数(含图片引用计数安全)
  - G2:DedupKey 加 File 变体,public.file-url 参与去重
  - 整个 insert_or_update_multi 包 unchecked_transaction
  - 5 个新测试
- **阶段 C 敏感过滤**(`ebe1d0e`):clipboard/filter.rs + handler + 设置接线
  - is_sensitive 纯函数,9 个高置信度模式(AWS/GitHub/GitLab/Slack/Stripe/Google/私钥)
  - extract_plain_text 从捕获格式提取纯文本
  - handler 步骤 4.5(空内容后/写DB前),读 settings.enable_sensitive_filter
  - 设置 4 处同步(Rust Settings + Default + 前端 Zod + SettingsPanel Toggle)
  - 依赖:Cargo.toml 加 regex = "1"
  - 不做:高熵检测(误报不可接受)/ 应用黑名单(P1)
- **阶段 A CI workflow**(`98e06c1`):.github/workflows/release.yml
  - push tag v* + workflow_dispatch 触发
  - matrix:macOS arm64 + macOS x86_64 + Windows
  - tauri-action + rust-cache + npm cache,草稿 release
  - ⚠️ 前置:用户需建 GitHub 仓库 + add remote + push tag

**M4 PR review 完成记录**(2026-08-09)

5 agent 并行(code-reviewer / silent-failure-hunter / type-design-analyzer / pr-test-analyzer / comment-analyzer),
2 Critical + 5 Important,全部修复(`8bb7acb`)。

- 🔴 **C1 已修**:insert_formats 返回 Result 传播错误(原静默吞错违反 §8,与 replace_formats 不一致)
- 🔴 **C2 已修**:settings/mod.rs 配置项计数 8→7(autostart 不在 Settings struct)
- 🟡 **I1 已修**:补 replace_formats 图片引用计数端到端测试(dedup_replaces_formats_drops_old_image)
- 🟡 **I2 已修**:补 extract_plain_text 4 个纯函数测试
- 🟡 **I3 已修**:handler 敏感过滤 try_state None 分支加 log::warn(与自动清理一致)
- 🟡 **I4 已修**:items.rs:10 hash 注释改为「M4 评估后决定不做」(原与决策矛盾)
- 🟡 **I5 已修**:default_true → default_enable_sensitive_filter(命名对称)

**验证基线**:cargo test 189 passed(原 159 + B 5 + C 25 + review 5)+ npm test 55 passed + clippy 零 warning

**M4 期间确立的技术决策**(待沉淀 constraints.md):
- 敏感过滤:只做固定前缀正则(gitleaks 精选),不做高熵检测(误报不可接受)
- 去重格式覆盖:命中去重时 formats 整体替换(同事务),防旧格式残留
- CI 打包:tauri-action 三平台,macos-latest 已是 ARM(双架构交叉编译)
- 无证书不阻断打包(macOS 手动放行 / Windows SmartScreen 警告)

## M4 UI 苹果审美改造完成记录(2026-08-09)

深色毛玻璃主题落地 + 三组件重做,实机多轮调校。

> 设计稿先行:三份 HTML 风格对比(深色毛玻璃/浅色系统/深灰折中),
> 用户选定深色①作为主方向。完整设计规格见 [[ui-design-spec]]。

- **Token 层**(`a51b93f`):
  - index.css 重写:深色①完整配色(shadcn hsl 变量换苹果系统色)+ UI 专用 token
    (缓动/时长/圆角分档/阴影/毛玻璃)+ 工具类(glass-window/mask-fade/scrollbar-thin/list-item hover)
  - tailwind.config.js 重写:接线 boxShadow.window / transitionTimingFunction / borderRadius 分档 / letterSpacing
  - main.tsx:固定 .theme-dark + .dark 双 class(主题切换架构预留)
- **ClipboardList 重做**:
  - CSS Grid 三列布局(auto 1fr auto)替代 flex(flex shrink-0 在 WebView 不可靠)
  - 胶囊选中态(bg-selected + inset 高光)+ hover(原生 CSS)+ 按压(active:scale)
  - 选中展开:文字 line-clamp-3(完整 title),图片缩略图放大 20→40px
  - 类型图标预留位(text/image/file/code/md),无图无文件时不渲染(不留空位)
  - 移除 copy_count 徽标(对用户无决策价值)
- **SpotlightPanel 重做**:半透明外壳 + 标题居中可拖拽 + 搜索框卡片化 + lucide 图标
- **SettingsPanel 重做**:半透明外壳 + 三栏标题居中 + 分组(通用/存储/行为)+ Toggle/Select 苹果化
  + autostart 加载占位(消除初始 false→true 跳变闪烁)
- **窗口**:tauri.conf.json 高度 480→600
- **新增依赖**:lucide-react 1.30(矢量图标库)

**实机调校(迭代修正,多轮)**:
- 🔴 backdrop-filter 滚动闪烁:macOS WKWebView 系统级限制,去掉 blur 保留 rgba 半透明
- 🔴 窗口灰色直角:#root 加 border-radius + overflow:hidden 裁剪
- 🔴 hover 不生效:Tailwind hover:bg-hover 未生成 + 手写门控规则全局覆盖 hover transparent
  → 删除门控 + 原生 CSS .list-item:hover 实现
- 🔴 Toggle 圆点跑到轨道外:Tailwind 任意值 [18px] 未生成 → 改内联 style transform
- 🔴 时间挤到下一行:flex shrink-0 在 WebView 不可靠 → 改 CSS Grid 固定列宽
- 🔴 选中文字只显示两行:JS slice(60) + CSS line-clamp 双重截断 → 选中时不 slice
- 🟡 透明度 0.72→0.85(实机反馈太透)
- 🟡 autostart Toggle 跳变:useState(false) 初始值 + 异步查询 → 加载占位

**M4 UI 期间确立的技术决策**(待沉淀 constraints.md):
- macOS WKWebView backdrop-filter 滚动闪烁:不用 blur,用 rgba 半透明替代
- Tailwind 任意值 [Npx] 在本项目配置下可能不生成:关键位移用内联 style
- Tailwind hover:bg-{自定义颜色} 可能不生成:hover 用原生 CSS 类实现
- 布局用 CSS Grid(auto 1fr auto)比 flex shrink-0 在 Tauri WebView 更可靠
- 主题架构预留:.theme-{dark/light/mid} class + Settings theme 字段(未实现切换)

**验证基线**:npm build ✅ + npm test 55 passed ✅ + typecheck ✅(无 Rust 改动)

## 多主题切换完成记录(2026-08-09)

三套主题(深色/浅色/折中)从预设计转正式,分段按钮选择器 + localStorage 防闪。

- **阶段 A Token 层**(`index.css` 重写 + 硬编码修复):
  - 共享 token(缓动/时长/圆角)抽到 `:root`,三套配色各自一个 `.theme-*` 块
  - `.theme-dark`(默认)/ `.theme-light`(浅色系统)/ `.theme-mid`(深灰折中+暖橙)
  - 新增 6 个语义化 token(解决跨主题硬编码颜色):`--divider` / `--card-raised` /
    `--toggle-thumb` / `--thumb-shadow` / `--selected-highlight` / `--scrollbar-thumb`
  - 修 7 处硬编码颜色:`border-white/5` / `border-white/10` / `bg-white/5` / `bg-white`(Toggle 圆点)/
    `rgba(255,255,255,0.12)`(选中高光)/ `rgba(0,0,0,0.2)`(圆点阴影)/ `.scrollbar-thin` 滚动条色
  - tailwind.config.js 加 `divider` / `card-raised` 颜色键接线
- **阶段 B 数据层**(两端同步):
  - Rust `Settings` struct 加 `theme: String` + `default_theme()` → `"dark"`
  - 前端 Zod `SettingsSchema` 加 `theme: z.enum(['dark','light','mid'])` + `ThemeName` 类型
  - 向后兼容:老 settings.json 无 theme 字段 → serde default 自动兜底 dark
- **阶段 C 切换逻辑**:
  - 新建 `src/core/theme.ts`:`applyTheme`(DOM class + localStorage)/ `getInitialTheme`(启动防闪)/ `isThemeName`
  - `main.tsx`:`applyTheme(getInitialTheme())` 替代硬编码 add('theme-dark')
  - `App.tsx`:settings theme 变化 → useEffect → applyTheme(纠正 localStorage 缓存)
- **阶段 D UI**:
  - 新建 `ThemeSwitcher.tsx`:分段按钮(macOS 系统设置风格),三选一,role=radiogroup
  - `SettingsPanel.tsx`「通用」组顶部加「外观」SettingRow
- **阶段 E 测试**:
  - `theme.test.ts`(8 测试):isThemeName 合法/非法、applyTheme class 设置/替换/写缓存、getInitialTheme 三分支
  - `ThemeSwitcher.test.tsx`(4 测试):渲染三选项、选中态 checked、点击触发 onChange、aria-label

**技术决策**:
- 启动防闪:localStorage 缓存优先(同步),settings.json 加载后纠正(异步)
- .dark class 始终保留(Tailwind darkMode 触发器,让 dark: 前缀生效)
- 实机调校透明度:浅色 0.82→0.88 / 折中 0.78→0.88(spec 原值偏透)
- 滚动条 hover 用 filter:brightness(1.3) 替代硬编码 hover 色(随主题自适应)

**验证基线**:cargo test 189 passed + npm test 67 passed(原 55 + theme 8 + switcher 4)+ npm build ✅ + clippy 零 warning

## UI 增强迭代完成记录(2026-08-09)

多主题切换之后的一轮 UI/功能增强迭代,11 个 commit。涵盖交互增强 + 存储管理 + 内容识别 + 类型筛选。

### 1. 双击回填(`26e9d77`)
- ClipboardList 加 onDoubleClick → handleCopyBack(与回车同路径)
- 底栏快捷键提示加「双击」
- 选中项文字高亮 text-primary(三套主题各自强调色)
- 设置标题层级修复:三等分 flex 替代 absolute(「设置」被「已保存」遮挡)

### 2. 存储统计 + 清除历史(`e3e4f28` + `910f816`)
- 后端:items.rs 加 count_by_kind(按 contentKind+subtype 分组)+ delete_all(清 DB+图片)
- models.rs 加 StorageStats / StorageByKind 结构
- get_storage_stats / clear_history command
- 前端:useStorageStats hook + StorageStatsPanel(6 个分类卡片 + 总条数 + 占用 + 清除按钮)
- 清除历史:二次确认(内联确认/取消)
- 设置面板分组顺序调整:通用 → 行为 → 存储(统计归入存储组)

### 3. 窗口透明度(`9acd6f1` + `c34f2b4`)
- index.css:--window-bg 拆分为 --window-rgb + --window-opacity(三套主题统一)
- Settings 加 window_opacity 字段(默认 85)+ 前端 Zod 同步
- theme.ts:applyWindowOpacity(动态修改 CSS 变量)
- 设置面板「通用」组加背景不透明度 Select(15%-100% 每 10% 一档)
- ⚠️ 修正:alpha = opacity/100(原反了,100%=完全不透明)

### 4. 内容类型智能识别(`a88c220`)
- classify.rs:detect_subtype 纯函数(link=纯 URL,markdown=≥2 种结构标记)
- Migration v3:加 content_subtype 列 + 存量回填
- 写入时识别:insert_or_update + insert_or_update_multi 都做识别
- 统计:count_by_kind 按 kind+subtype 分组,link/markdown 从 text 拆出
- 前端:StorageStatsPanel 链接/Markdown 卡片去掉半透明(正式启用)

### 5. 类型图标 + 文件/图片路径展示(`151663f`)
- 列表项类型图标:文字Type/文件FileText/混合Layers/链接Link2/Markdown FileCode
  (与统计卡片图标完全对应,图片用缩略图)
- ClipboardItem 加 imagePath + fileUrl 字段
- fill_image_fields 填原图绝对路径 / fill_file_url 从 formats 表读 file-url
- 列表项图片/文件两行展示(标题+路径)

### 6. 文件分类修复(`0bc3be9`)
- 🔴 macOS 复制文件时附带的 tiff(Finder 图标预览)导致误判 Mixed/Image
- capture_multi:有 file-url 时跳过 image 捕获
- classify_content_kind:file + text 不算 Mixed(macOS 附带的纯文本是文件名)
- 只有 file + rich(rtf/html)才算 Mixed

### 7. 类型筛选 tag(`41032f2` + `16610b9`)
- 后端:list_items 加 kinds 参数,支持 content_kind + content_subtype 混合筛选
  (subtype:link / subtype:markdown 用 OR 组合;text 排除有子类型的记录)
- 前端:useClipboardItems 加 kindFilter 状态 + reload + listen 匹配检查
- UI:SpotlightPanel 搜索框下方 6 个 tag(文字/图片/文件/混合/链接/Markdown)
  选中高亮,多选叠加,全未选=不过滤
- 修复:setKindFilter 先更新 ref 再 reload(防筛选值滞后一帧)

**技术决策**:
- 文件场景的 image(tiff/icns)是 Finder 图标预览:有 file-url 时跳过 image 捕获
- file + 纯文本不算 Mixed:macOS 附带的纯文本是文件名表示,不是独立内容
- 内容子类型写入时识别:规则不常变,一次识别永久存储(查询时不重复计算)
- 窗口透明度拆分 RGB+opacity:主题色和透明度独立变化
- 设置标题三等分 flex:absolute 定位会被后续元素遮挡

**验证基线**:cargo test 209 passed(+20: classify 14 + migration 2 + delete_all/count 4)
+ npm test 67 passed + npm build ✅ + clippy 零 warning

## 本工程开发历史

### M0 架构清理 + Spotlight UI 骨架（2026-08-05）

8 阶段施工,从「Tauri + Python sidecar 双进程模板」改造为「纯 Rust 单进程剪贴板工具骨架」:

- **阶段 A**(`06f1892`):归档旧 M0 规划 + 落盘新 M0 规划 + sidecar 方法论删前留档
- **阶段 B**(`93da773`):删除 backend/ + sidecar.rs + build-sidecar.sh(3409 行)
- **阶段 C**(`3d30b48`):Rust 侧改造(error.rs + commands/ping + Spotlight 窗口配置)
  - 踩坑:transparent:true 必须配合 Cargo.toml tauri features=["macos-private-api"]
  - 踩坑:generate_handler! 宏对模块路径解析有限制,改 use 别名裸名注册
- **阶段 D+E**(`4a03c33`):前端换栈 Tailwind+shadcn,删 axios/AntD/路由,重建 invoke 通信层
  - 踩坑:CSS 文件 JSDoc 风格多行注释(含 `bg-*`)让 PostCSS parser 报错,精简单行注释
- **阶段 F**(`ff3b92d`):清理配置/脚本/环境变量(.env.example + init-project.sh + .gitignore)
- **阶段 G**(`29af37f`):文档同步(AGENTS/CLAUDE/README + wiki 架构文档,定位改为剪贴板应用)
- **阶段 H**(`7a3bb7a`):启动实测验证,修复探活竞态 bug
  - 实测发现:静态检查全过但 GUI 实测暴露真实 bug(§1.5 价值)
  - 根因:React StrictMode 双渲染 + 成功路径漏 clearTimeout(deadline),ping 成功后超时 setTimeout 覆盖了 ready
  - 修复:ping 成功后立即 clearTimeout(deadline)
  - 验证:UI 正确显示「● 后端就绪」,Console 零错误

**关键技术决策**(产品定位明确后重新规划):
- UI:Tailwind + shadcn/ui(换掉 AntD,轻量 + 键盘导航友好)
- 窗口:Spotlight 风格(无边框/透明/置顶/失焦隐藏,600×480)
- 剪贴板(M1+):平台原生 API(macOS NSPasteboard / Windows Win32)
- 监听(M1+):原生事件(changeCount / AddClipboardFormatListener)
- 存储(M1+):rusqlite + refinery
- 前端通信:Tauri invoke(换掉 axios,无 HTTP)
- 路由:删除 react-router-dom(单窗口不需要)

### 工程初始化（2026-08-05）

- 从模板 `tauri-sidecar-template` 通过 `scripts/init-project.sh` 复制创建
- 占位符替换：tauri-sidecar-template → clipboard-manager（19 个文件）
- git init + 首次 commit `ac02fed`
- 规划 M0 架构清理任务

---

## 以下为模板母体历史（本工程出身参考，不再维护）

> 本工程从模板复制，模板的开发历史保留如下供追溯。本工程自己的进度在上方维护。

## 历史版本

### v1.2.0 — 新增版本 bump 规则 + 同步锚点机制（2026-07-03）

完善回流机制：明确「谁 bump / 何时 bump / bump 同步哪些位置」+「版本号作为模板与子工程的同步锚点」。
- `template-evolution-guide.md` 版本约定节扩充（bump 触发表 + 同步位置清单 + 同步锚点说明）

### v1.1.0 — 回流 pixel-matrix Stage 5A（2026-07-03）

子工程 pixel-matrix 剥离 Python sidecar 改 Rust 原生后端，提炼迁移方法论回流模板：
- 新增 `wiki/process/sidecar-to-rust-migration-guide.md`（sidecar→Rust 迁移指南）
- `constraints.md` 新增「Tauri v2 setup 时序坑」节（setup 内 emit 会被丢弃）
- 索引同步（index.md 速查表 + 分类导航）

**回流 commit**：`3659169`。详见 CHANGELOG.md [1.1.0]。

### v1.0.0 — 模板初始落地（2026-06-29）

从 ai-audiobook 项目沉淀的通用工程模板，10 个 commit：
1. Tauri 2 + React + TS 工程骨架
2. Python sidecar 进程管理（三重兜底，S 级）
3. FastAPI sidecar 骨架（health/status + core 分层 + alembic）
4. 示例 CRUD（items）+ 第一个 migration
5. 前端 core 骨架（api client + 启动守卫 SetupGate）
6. 示例 HomePage（调 /api/items 全链路）+ UI 组件
7. 工程质量基线（ruff/mypy/pytest + conftest + 示例测试）
8. PyInstaller sidecar 打包脚本（onefile）
9. 知识库骨架 + 工程方法论套件 + 进化机制
10. README 进化模式说明 + CHANGELOG

**验证**：tauri dev 起空窗 + sidecar health 200 + items CRUD 全链路 + 打包 binary 可运行。
