"""
路径管理 — 应用根目录、用户数据目录、临时文件目录

所有跨模块共享的路径计算都集中在此，禁止在业务代码里硬编码路径。

这是模板的 S 级资产：frozen/开发双模式路径解析解决 PyInstaller 打包路径漂移
（开发期 `__file__` 源码树 vs 打包后 `_MEIPASS` 解压目录 vs `APP_ROOT` tauri resource_dir）。
"""

from __future__ import annotations

import os
from functools import lru_cache
from pathlib import Path

# ============================================================
# 应用安装目录（项目根目录 / PyInstaller 解压目录）
# ============================================================

# PyInstaller onefile 打包后，所有 --add-data 资源解压到 sys._MEIPASS，
# 此时 __file__ 指向 _MEIPASS 内的 core/paths.py，.parent.parent.parent 不再
# 是真实项目根。frozen 探测把 APP_DIR 统一指向 _MEIPASS，派生常量随之适配。
# 开发期（非 frozen）保持源码树原逻辑。


def _is_frozen() -> bool:
    """是否运行在 PyInstaller 打包环境（onefile 模式）。

    frozen 时 sys 含 _MEIPASS 属性（解压目录）。开发期为 False。
    """
    import sys

    return hasattr(sys, "_MEIPASS")


def _compute_app_dir() -> Path:
    """计算应用根目录（单一真理源，frozen/开发双模式）。

    - 打包后：sys._MEIPASS（PyInstaller 解压目录，所有 --add-data 资源落在此）
    - 开发期：源码树项目根（core/paths.py 往上 3 级）
    """
    if _is_frozen():
        import sys

        return Path(sys._MEIPASS)  # type: ignore[attr-defined]  # frozen 时必有
    return Path(__file__).resolve().parent.parent.parent


def _compute_backend_dir() -> Path:
    """计算 backend 目录（frozen/开发双模式）。

    - 打包后：sys._MEIPASS。PyInstaller spec 把 backend 的 data 资源（alembic.ini /
      alembic/ 等）解压到 _MEIPASS 根（spec datas 目标为 '.'），所以 frozen 下
      backend 目录 == _MEIPASS。.py 代码本身在 PYZ 归档里，不占文件系统路径。
    - 开发期：APP_DIR / "backend"（源码树 backend/ 目录）
    """
    if _is_frozen():
        import sys

        return Path(sys._MEIPASS)  # type: ignore[attr-defined]
    return _compute_app_dir() / "backend"


APP_DIR: Path = _compute_app_dir()
BACKEND_DIR: Path = _compute_backend_dir()


def _app_root() -> Path:
    """应用自包含资源根（resources/ 目录定位用，打包适配）。

    - 打包后：APP_ROOT 环境变量（Rust sidecar.rs 注入 tauri resource_dir，
      resources/ 下自包含资源经 tauri bundle.resources 分发到此）
    - 开发期：APP_DIR（源码树项目根，resources/ 在项目根下）

    区别于 APP_DIR：frozen 下 APP_DIR=_MEIPASS（PyInstaller 解压目录，只有打入 binary
    的代码/data），而应用自包含资源走 tauri resource_dir 分发，不在 _MEIPASS 里。
    两者通过 APP_ROOT 环境变量对齐。

    Raises:
        RuntimeError: frozen 模式下 APP_ROOT 环境变量未注入。这是异常情况
            （Rust sidecar.rs spawn 时必须注入），fast-fail 在启动期暴露。
    """
    if _is_frozen():
        env_val = os.environ.get("APP_ROOT")
        if env_val:
            return Path(env_val)
        # frozen 下 APP_ROOT 未注入 = Rust spawn 链路异常，fast-fail 暴露真正根因
        raise RuntimeError(
            "frozen 模式下 APP_ROOT 环境变量未注入。"
            "Rust sidecar.rs spawn Python 时必须注入 APP_ROOT 指向 tauri resource_dir。"
            "请检查 sidecar.rs find_python_runner 的 Binary 分支是否注入了 APP_ROOT_ENV。"
        )
    return APP_DIR


APP_ROOT: Path = _app_root()


@lru_cache(maxsize=1)
def get_app_dir() -> Path:
    """应用安装目录（开发期为项目根目录，发布期为 .app 内部）"""
    return APP_DIR


# ============================================================
# 用户数据目录（跨应用版本持久化）
# ============================================================

# 环境变量名：Tauri（生产）通过此变量注入 appDataDir，实现开发/生产数据隔离。
# - 生产（打包 .app）：Rust 端注入 SIDECAR_DATA_DIR=<appDataDir>
#   （macOS: ~/Library/Application Support/<identifier>/）
# - 开发（tauri dev）：Rust 未注入 → 回退 ~/.<app-name>（开发数据零迁移）
# - 测试：monkeypatch 此环境变量重定向到 tmp_path
#
# ⚠️ 必须与 Rust 侧 sidecar.rs DATA_DIR_ENV 常量对齐（当前都是 "SIDECAR_DATA_DIR"）。
USER_DATA_DIR_ENV = "SIDECAR_DATA_DIR"
# 开发兜底目录名（相对 home）：开发期回退路径，也是升级迁移来源。
_LEGACY_DEV_DATA_DIRNAME = ".clipboard-manager"


@lru_cache(maxsize=1)
def _resolve_user_data_dir() -> Path:
    """运行时解析用户数据根目录（单一真理源）。

    优先级：
    1. 环境变量 SIDECAR_DATA_DIR（由 Tauri Rust 端注入 appDataDir）—— 生产环境
    2. 回退 ~/.<app-name> —— 开发环境（零迁移）

    所有派生路径（DATABASE_PATH / ...）必须经由本函数派生，
    禁止再出现第二个 home / appDataDir 计算源（消除双源耦合）。
    """
    env_val = os.environ.get(USER_DATA_DIR_ENV)
    if env_val:
        return Path(env_val).expanduser()
    return Path(os.path.expanduser("~")) / _LEGACY_DEV_DATA_DIRNAME


# 模块级常量保留为向后兼容入口，但值由 _resolve_user_data_dir() 派生。
# 注意：这些常量在首次 import 时绑定一次，环境变量须在此之前设置。
# 测试请 monkeypatch _resolve_user_data_dir 而非直接改常量。
USER_DATA_DIR: Path = _resolve_user_data_dir()
LOGS_DIR: Path = USER_DATA_DIR / "logs"
CACHE_DIR: Path = USER_DATA_DIR / "cache"

DATABASE_PATH: Path = USER_DATA_DIR / "data.db"
CONFIG_PATH: Path = USER_DATA_DIR / "config.json"


def get_user_data_dir() -> Path:
    return USER_DATA_DIR


def get_database_path() -> Path:
    return DATABASE_PATH


def get_logs_dir() -> Path:
    return LOGS_DIR


def get_cache_dir() -> Path:
    """缓存目录（上传残留 / 临时输出等）。数据管理清理用。"""
    return CACHE_DIR


def ensure_user_dirs() -> None:
    """首次启动时创建用户数据目录结构。"""
    for d in (USER_DATA_DIR, LOGS_DIR, CACHE_DIR):
        d.mkdir(parents=True, exist_ok=True)
