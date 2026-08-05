# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec — sidecar binary（最简单 onefile 模式）。

产物：单个可执行文件 `sidecar`（含 Python runtime + FastAPI + DB 栈 + backend 代码）。
由 scripts/build-sidecar.sh 拷贝到 resources/bin/，靠 tauri.conf.json bundle.resources
目录 map 进 .app bundle。Rust 端 sidecar.rs 的 SIDECAR_BINARY_RELPATH 探测它。

打包前置：
  - Python 环境已装 pyinstaller（pip install pyinstaller）
  - 运行：pyinstaller backend/build/sidecar.spec --noconfirm
  - 或：./scripts/build-sidecar.sh

业务工程起步时，若引入重依赖（如 torch / 大模型库），建议：
  - 重依赖在函数内 lazy import（避免启动路径触发）
  - 或参考 ai-audiobook 的「进程级拆分」（轻量 sidecar + 重 worker 分离）
"""

from pathlib import Path

from PyInstaller.utils.hooks import collect_all

# backend 目录（spec 文件在 backend/build/，SPECPATH = backend/build/）
BACKEND_DIR = Path(SPECPATH).parent  # type: ignore[name-defined]  # SPECPATH 由 PyInstaller 注入

# ============================================================
# collect_all：收 web/db 栈 + 运行时依赖
# ============================================================

_collect_targets = [
    # --- SQLAlchemy 异步栈（aiosqlite 动态导入，PyInstaller 扫不到，必须 collect_all）---
    "aiosqlite",  # SQLite 异步驱动
    "greenlet",  # SQLAlchemy async 依赖（协程到同步桥接）
    "sqlalchemy",
    "sqlalchemy.dialects.sqlite",
    "sqlalchemy.ext.asyncio",
    # --- Web 栈 ---
    "fastapi",
    "uvicorn",
    "pydantic",
    "httptools",  # uvicorn 推荐的 HTTP 解析器（如有装）
    # --- 其他运行时依赖 ---
    "alembic",  # DB migration（启动时 database.py 跑 upgrade head）
    "jinja2",  # FastAPI/uvicorn 模板依赖
]

datas = []
binaries = []
hiddenimports = []
for pkg in _collect_targets:
    pkg_datas, pkg_binaries, pkg_hidden = collect_all(pkg)
    datas += pkg_datas
    binaries += pkg_binaries
    hiddenimports += pkg_hidden

# 显式补充 backend 自身的非 .py 运行时资源（PyInstaller 自动收 .py，但不收 .ini/alembic/）
datas += [
    # alembic 迁移（启动时 database.py 自动跑 upgrade head，必须带入）
    (str(BACKEND_DIR / "alembic"), "alembic"),
    (str(BACKEND_DIR / "alembic.ini"), "."),
]

# ============================================================
# Analysis
# ============================================================

a = Analysis(
    [str(BACKEND_DIR / "sidecar.py")],
    pathex=[str(BACKEND_DIR)],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    runtime_hooks=[],
    excludes=[
        # 排除 conda 环境里装着但 sidecar 运行时不用的大包（缩小 binary）。
        # 业务工程若需要这些包，从此处移除并加入 _collect_targets。
        "numpy",
        "scipy",
        "pandas",
        "matplotlib",
        "PIL",
        "sklearn",
    ],
    noarchive=False,
)

pyz = PYZ(a.pure)

# onefile 模式：单文件可执行（EXE 包含全部 binaries + datas）。
# 产物名 sidecar（与 Rust sidecar.rs SIDECAR_BINARY_RELPATH 对齐）。
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,  # onefile：binaries + datas 直接进 EXE
    [],
    name="sidecar",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
