# Clipboard Manager

> Tauri 2 + React 18 + Python FastAPI sidecar 通用工程模板。
> 持续进化的母体——新工程从模板复制起步，开发成果回流让模板进化。

## 这是什么

一个开箱即用的「Tauri 桌面应用 + Python 后端 sidecar」工程骨架，附带从真实项目沉淀的
工程方法论（20 条通用工程纪律 + AI 协作风格 + 流程清单）。

**适用场景**：任何需要「桌面壳（Tauri）+ 本地后端服务（Python）」的应用，如：
- AI 工具（调云端模型 + 本地数据处理）
- 开发者工具（本地服务 + GUI）
- 数据处理应用（Python 计算 + 前端展示）

## 包含什么

### 代码骨架（可直接跑）

| 层 | 内容 | 关键资产 |
|----|------|---------|
| **Tauri (Rust)** | sidecar 进程管理 | ⭐ 三重兜底（Child handle → PID 文件 → 端口 lsof）+ 健康检查 + 生产/开发双模式 |
| **Python sidecar** | FastAPI 后端 | ⭐ frozen/开发双模式路径解析 + SQLite 加固 + alembic + SSE 事件总线 |
| **React 前端** | Ant Design 5 | ⭐ API client（ApiResponse 拆包）+ 启动守卫 SetupGate + 错误码镜像契约 |
| **打包** | PyInstaller onefile | sidecar.spec + build-sidecar.sh |
| **示例** | items CRUD 全链路 | api→service→model 三层 + 前端 HomePage 调用 |

### 方法论套件（`wiki/`）

- ⭐ **20 条通用工程纪律**（`engineering-principles.md`）—— 任何软件工程适用
- AI 协作风格 13 条 + 分块逐项决策方法
- 会话收尾 8 步清单 + Code Review 三套清单
- 双工具同步契约（CLAUDE.md ↔ AGENTS.md）+ pre-commit hook
- 知识库内容规范（防膨胀）

## 快速开始

### 1. 从模板创建新工程

```bash
cd clipboard-manager
chmod +x scripts/init-project.sh   # 首次赋予执行权限
./scripts/init-project.sh           # 交互式创建（复制 + 改名 + git init 一步到位）
```

脚本会问你 4 项（有默认值，回车确认）：新工程目录 / 工程名 / bundle identifier / 显示名。
详见 `scripts/README.md`（脚本说明书）。

> 手动改名容易漏（占位符散落 19+ 文件），务必用脚本。

### 2. 开发环境

```bash
# 前端依赖
npm install

# Python 环境（复用已有 conda/venv，或新建）
# 在 .env 设置 SIDECAR_DEV_PYTHON 指向你的 Python 解释器
cp .env.example .env
# 编辑 .env：SIDECAR_DEV_PYTHON=/path/to/your/python

# 安装后端依赖
<SIDECAR_DEV_PYTHON> -m pip install -r backend/requirements.txt
```

### 3. 运行

```bash
# 开发模式（自动 spawn sidecar + 前端热更新）
npm run tauri dev

# 验证 sidecar
curl http://localhost:9880/api/health
# → {"success":true,"data":{"status":"ok",...}}

# 测试 items CRUD
curl -X POST http://localhost:9880/api/items -H "Content-Type: application/json" -d '{"name":"test"}'
```

### 4. 打包

```bash
# 打包 sidecar binary（PyInstaller onefile → resources/bin/sidecar）
PYTHON=/path/to/your/python ./scripts/build-sidecar.sh

# 打包桌面应用（tauri build → .app / .exe / .deb）
npm run tauri build
```

### 5. 测试 + 质量检查

```bash
cd backend
python -m pytest              # 18 tests
python -m ruff check .        # lint
python -m mypy core api services sidecar.py  # 类型检查
```

---

## 🔴 进化模式（模板是持续进化的母体）

模板不是一次性产物，而是「中央模板 + 分支项目 + 回流进化」的持续维护母体：

```
        ┌─ 新工程A（从模板 v1.0 复制）─┐ 成果提炼回流 ┐
模板工程 ─┤                              ├────────────→│ 模板不断进化（版本递增）
（母体） └─ 新工程B（从模板 v1.1 复制）─┘              │ 越来越好用
    ↑                                                  │
    └──── 每次回流后版本+1 ────────────────────────────┘
```

**你是贡献者**：新工程开发中发现的通用成果（工程纪律 / 通用工具 / 踩坑经验）应回流模板。

回流机制详见 `wiki/process/template-evolution-guide.md`，回流记录见 `CHANGELOG.md`。

### 版本约定

- `vMAJOR.MINOR.PATCH`（语义化版本）
  - **MAJOR**：架构骨架大改
  - **MINOR**：回流新增（决策逻辑/工程纪律/通用工具）
  - **PATCH**：修正/小优化

新工程从某版本复制时，在此记录"基于模板 vX.Y.Z 起步"，便于判断该回流哪些。

---

## 工程结构

```
├── AGENTS.md / CLAUDE.md    AI 工作说明（双工具同步）
├── README.md / CHANGELOG.md 模板说明 + 版本追溯
├── src/                     React 前端（core 骨架 + pages/components）
├── src-tauri/               Tauri Rust（⭐ sidecar.rs 三重兜底）
├── backend/                 Python sidecar（api/services/core + tests）
├── scripts/                 打包脚本 + hooks
├── resources/bin/           PyInstaller binary 落点
└── wiki/                    ⭐ 工程方法论套件（20 条工程纪律 + 流程清单）
```

详细架构 → `wiki/architecture/overview.md`。
技术红线 → `wiki/architecture/constraints.md`。

---

## 技术栈

- **前端**：React 18 + Ant Design 5 + TypeScript + Vite + axios + zustand + react-router
- **桌面壳**：Tauri 2（Rust）
- **后端**：Python 3.10 + FastAPI + SQLAlchemy 2.0 async + SQLite + alembic
- **打包**：PyInstaller（sidecar）+ Tauri bundle（桌面应用）

## License

MIT
