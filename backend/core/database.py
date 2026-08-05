"""
SQLAlchemy 2.0 async engine + session 管理

- 数据库文件：<user_data_dir>/data.db
- 使用 aiosqlite 作为 async 驱动
- 提供 async_session 工厂 + FastAPI 依赖注入
- 启用 SQLite FK 约束 + WAL + busy_timeout（并发写加固）
"""

from __future__ import annotations

from collections.abc import AsyncGenerator
from typing import Any

from sqlalchemy import event
from sqlalchemy.ext.asyncio import (
    AsyncEngine,
    AsyncSession,
    async_sessionmaker,
    create_async_engine,
)
from sqlalchemy.orm import DeclarativeBase

from core.paths import DATABASE_PATH, ensure_user_dirs


class Base(DeclarativeBase):
    """所有 ORM 模型的基类。"""


# ============================================================
# Engine + Session 工厂
# ============================================================

# 模块级全局 engine，应用启动时创建，关闭时释放。
# 单元测试时通过 init_engine(db_url) 替换为内存数据库。
_engine: AsyncEngine | None = None
_session_factory: async_sessionmaker[AsyncSession] | None = None


def _enable_sqlite_pragmas(dbapi_conn: Any, _connection_record: Any) -> None:
    """SQLite 连接级 PRAGMA 配置（每个新连接执行）。

    3 个 PRAGMA：
    - ``foreign_keys=ON``：默认 OFF，开启后 ForeignKey + ondelete 才真实生效（否则装饰性）。
    - ``journal_mode=WAL``：Write-Ahead Logging，读连接不阻塞写连接。
      **in-memory DB 静默回退为 "memory"**（SQLite 行为，不报错）。
    - ``busy_timeout=5000``：拿写锁失败时内部重试 5s 才报 ``database is locked``，
      兜底瞬时竞争窗口。
    """
    cursor = dbapi_conn.cursor()
    cursor.execute("PRAGMA foreign_keys=ON")
    cursor.execute("PRAGMA journal_mode=WAL")
    cursor.execute("PRAGMA busy_timeout=5000")
    cursor.close()


def _build_engine(db_url: str) -> AsyncEngine:
    engine = create_async_engine(
        db_url,
        echo=False,
        future=True,
        # SQLite 单连接足够（本地 sidecar 单实例）
        pool_pre_ping=True,
    )
    # SQLite 强制 FK + WAL + busy_timeout（其他 DB 默认开启/不适用，无需 listener）
    if db_url.startswith("sqlite"):
        event.listens_for(engine.sync_engine, "connect")(_enable_sqlite_pragmas)
    return engine


def init_engine(db_url: str | None = None) -> None:
    """
    初始化全局 engine + session 工厂。

    Args:
        db_url: SQLAlchemy 连接串。None 时使用默认 <user_data_dir>/data.db。
                测试时可传 "sqlite+aiosqlite:///:memory:"。
    """
    global _engine, _session_factory
    if db_url is None:
        ensure_user_dirs()
        db_url = f"sqlite+aiosqlite:///{DATABASE_PATH}"

    _engine = _build_engine(db_url)
    _session_factory = async_sessionmaker(
        bind=_engine,
        class_=AsyncSession,
        expire_on_commit=False,
        autoflush=False,
    )


async def dispose_engine() -> None:
    """应用关闭时释放连接池。"""
    global _engine, _session_factory
    if _engine is not None:
        await _engine.dispose()
    _engine = None
    _session_factory = None


def get_engine() -> AsyncEngine:
    if _engine is None:
        init_engine()
    assert _engine is not None
    return _engine


def get_session_factory() -> async_sessionmaker[AsyncSession]:
    if _session_factory is None:
        init_engine()
    assert _session_factory is not None
    return _session_factory


# ============================================================
# FastAPI 依赖注入
# ============================================================


async def get_session() -> AsyncGenerator[AsyncSession, None]:
    """FastAPI Depends 用的 session 工厂，自动 commit/rollback。"""
    factory = get_session_factory()
    async with factory() as session:
        try:
            yield session
            await session.commit()
        except Exception:
            await session.rollback()
            raise


async def init_db() -> None:
    """首次启动时创建/升级 schema。

    路径 dispatch：
    - in-memory DB（:memory:，测试用）：Base.metadata.create_all
      快速 + 无 alembic migration 开销 + in-memory 无脏数据需清理
    - 文件 DB（生产 <user_data_dir>/data.db）：alembic upgrade head
      正式版本管理 + 老脏数据由 data migration 清理
    """
    from core import models  # noqa: F401  确保模型已注册

    engine = get_engine()

    # 用 SQLAlchemy 标准 engine.url.database 判断 in-memory（不靠字符串子串）：
    # - sqlite+aiosqlite:///:memory:  → database == ':memory:'
    # - sqlite+aiosqlite://            → database is None（无路径）
    # - sqlite+aiosqlite:///path.db    → database == 'path.db'（文件 DB，走 alembic）
    if engine.url.database in (None, "", ":memory:"):
        async with engine.begin() as conn:
            await conn.run_sync(Base.metadata.create_all)
        return

    # 生产文件 DB：走 alembic upgrade head（版本管理 + data migration）
    await _run_alembic_upgrade_head()


async def _run_alembic_upgrade_head() -> None:
    """调 alembic.command.upgrade(cfg, "head")（sync 阻塞调用，包到工作线程）。

    alembic env.py 从 core.paths.DATABASE_PATH 派生 sync URL，无需在此注入。
    用 asyncio.to_thread 避免阻塞 event loop（init_db 在 async lifespan 中调用）。
    """
    import asyncio

    from alembic.config import Config

    from alembic import command
    from core.paths import BACKEND_DIR  # frozen 探测已支持

    alembic_ini = BACKEND_DIR / "alembic.ini"

    if not alembic_ini.is_file():
        raise RuntimeError(
            f"alembic.ini not found: {alembic_ini}\n"
            "生产 DB 必须用 alembic 管理 schema，请确认 alembic.ini 存在。"
        )

    alembic_cfg = Config(str(alembic_ini))

    # 显式设置绝对路径 script_location，消除 cwd 依赖。
    # alembic.ini 里 script_location = alembic（相对路径），默认相对 cwd 解析。
    # 开发期 cwd=backend/ 凑巧能工作；但打包后 PyInstaller binary 的 cwd 不确定
    # （可能是 / 或 home），相对路径会失败 → 启动卡在 alembic。
    alembic_cfg.set_main_option("script_location", str(BACKEND_DIR / "alembic"))

    # alembic.command.upgrade 是 sync 阻塞调用（建表 / 跑 migration）
    # 包到工作线程避免阻塞 event loop
    await asyncio.to_thread(command.upgrade, alembic_cfg, "head")
