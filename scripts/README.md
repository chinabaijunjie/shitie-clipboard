# scripts/ 说明书

> 本目录是模板的工具脚本目录。**给人和大模型看的说明书**——看不懂脚本代码时，读这里。

---

## init-project.sh — 创建新工程（核心脚本）

### 它做什么

基于本模板创建一个新工程：**复制模板目录 → 批量替换占位符 → 初始化 git**，一步到位。

### 什么时候用

当你想基于模板开始一个新项目时（如「AI 图片工作站」）。**不需要手动 cp + 改名**，跑这个脚本就行。

### 怎么用

```bash
# 1. 进入模板工程根目录（不是 scripts/ 目录！）
cd ~/Documents/infra/clipboard-manager

# 2. 赋予执行权限（首次用，之后不用重复）
chmod +x scripts/init-project.sh

# 3. 运行
./scripts/init-project.sh
```

脚本会**交互式问你 4 项**（有默认值，回车确认即可）：

| 问题 | 说明 | 示例 | 默认值 |
|------|------|------|--------|
| 新工程目录路径 | 新工程放哪 | `~/Documents/infra/ai-image-studio` | 无（必填）|
| 工程名 (kebab-case) | 全小写+连字符，用于 package/cargo/data-dir/storage | `ai-image-studio` | 从目录名推导 |
| Bundle identifier | 应用唯一身份证（反向域名） | `com.baijunjie.ai-image-studio` | `com.example.<工程名>` |
| 产品/显示名 | 窗口标题/品牌名（可中文/空格）| `AI 图片工作站` | 从工程名推导成 Title Case |

回答后脚本会显示替换预览，确认 `[y/N]` 后执行。

### 创建后做什么

脚本结束时会打印指引，简化版：

```bash
cd <你的新工程目录>
cp .env.example .env          # 配置 Python 解释器路径
npm install                   # 装前端依赖
npm run tauri dev             # 跑起来（会自动 spawn sidecar）
```

---

### 占位符替换说明（给大模型/想理解机制的人）

脚本替换 3 组占位符（替换范围已实测扫描确认）：

| 模板占位符 | 替换成 | 含义 | 漏改后果 |
|-----------|--------|------|---------|
| `clipboard-manager` | `<工程名>` | package/cargo 名、数据目录名、localStorage 前缀 | 工程名还是模板；多工程数据可能混 |
| `com.example.clipboard-manager` | `<identifier>` | Tauri bundle identifier（决定数据目录路径）| 数据目录混用 |
| `Clipboard Manager` | `<显示名>` | 窗口标题、APP_NAME、文档标题 | 显示名还是模板 |

**自动覆盖的派生占位符**（无需单独处理，是上面 3 个的子串）：
- `.clipboard-manager`（带点前缀的数据目录名）→ 由 `clipboard-manager` 替换覆盖
- `clipboard-manager:`（localStorage 前缀带冒号）→ 由 `clipboard-manager` 替换覆盖

**不替换的**（故意保留）：
- `app_lib`（Rust crate 内部名，用户看不到，不影响功能，多工程各自编译不冲突）

### 脚本排除的文件（不复制到新工程）

复制时排除这些（构建产物 / 本地配置 / git 历史）：

| 排除项 | 原因 |
|--------|------|
| `.git` | 新工程是独立仓库，不要模板的提交历史 |
| `node_modules` | 重新 `npm install`，体积大 |
| `src-tauri/target` | Rust 编译产物，重新编译 |
| `resources/bin/sidecar` | 模板验证时打的 binary，新工程要重新打包 |
| `dist` | Vite 构建产物 |
| `__pycache__` / `.pytest_cache` / `.mypy_cache` / `.ruff_cache` | Python 缓存 |
| `.env` / `.env.local` | **本地配置（含 Python 路径），不泄露到新工程** |
| `.DS_Store` | macOS 垃圾文件 |

> `resources/bin/` 目录本身会保留（加 `.gitkeep`），因为它是打包 binary 的落点。

---

### 常见问题

**Q: identifier 是什么？为什么必须改？**
A: 应用的唯一身份证号（反向域名格式 `com.公司.产品`），决定数据目录路径
（macOS: `~/Library/Application Support/<identifier>/`）。不改的话，多个工程的数据会混在同一个目录。

**Q: 工程名为什么必须是 kebab-case？**
A: 它会用在 URL/storage-key/data-dir 等场景，kebab-case（全小写+连字符）兼容性最好。
脚本会校验格式，不合规会报错。

**Q: identifier 以 .app 结尾会怎样？**
A: macOS Finder 会把数据目录误判为应用包（打不开）。脚本会警告，建议改掉。

**Q: 脚本报「必须在模板工程根目录执行」？**
A: 你可能在子目录跑了。`cd` 到 `clipboard-manager` 根目录（含 `package.json` 的那个）再跑。

**Q: git commit 失败？**
A: 可能没配置 git user.name/email。脚本会警告但不阻塞，你进新工程后手动 `git commit` 即可。

---

## build-sidecar.sh — 打包 sidecar binary

打包 Python sidecar 成可执行文件（PyInstaller onefile）。详见 `backend/build/sidecar.spec`。

```bash
PYTHON=/path/to/your/python ./scripts/build-sidecar.sh
# 产物：resources/bin/sidecar
```

## hooks/pre-commit — 双工具同步检查

检查 `CLAUDE.md` ↔ `AGENTS.md` 同步状态。安装方式（git hook 软链）：

```bash
cd <工程根目录>
ln -sf ../../scripts/hooks/pre-commit .git/hooks/pre-commit
```

详见 `wiki/process/dual-tool-sync.md`。
