"""Alembic 环境配置（sync engine 模式）。

设计决策：
- 用 sync engine（不是 async）：migration 是一次性操作，不需要 async；alembic async
  模式需要 run_sync 包裹 + event loop 管理，复杂度高且无收益。
- env.py 优先读 alembic.ini 的 sqlalchemy.url；若为空则从 core.paths.DATABASE_PATH
  派生 sync URL，避免 alembic.ini 写死 URL 与 paths.py 漂移。
- target_metadata = Base.metadata，import 所有 ORM 模型让 metadata 注册。
- compare_type + compare_server_default：让 autogenerate 检测类型 / 默认值变化。
- render_as_batch=True：SQLite 必须（不支持 ALTER COLUMN，batch mode 用 copy-table 模拟）。

启动入口：
- 命令行（cwd = backend/）：cd backend && <python> -m alembic upgrade head
- 生产 sidecar：sidecar.py:init_db() 编程式调 alembic.command.upgrade(cfg, "head")
"""

from __future__ import annotations

from logging.config import fileConfig

from sqlalchemy import engine_from_config, pool

from alembic import context

# ============================================================
# sys.path 注入（编程式调用入口必需；命令行 alembic 走 alembic.ini prepend_sys_path）
# ============================================================
# alembic.ini 的 prepend_sys_path=. 只在 alembic CLI 模式生效；
# 编程式调用（sidecar.py:init_db → alembic.command.upgrade）用 Config(str(ini_path))
# 构造时不处理 ini 的 prepend_sys_path，所以 env.py 必须自己注入 sys.path，
# 让下面 `from core.xxx import` 能解析到 backend/core/。
import sys
from pathlib import Path

_BACKEND_DIR = Path(__file__).resolve().parent.parent
if str(_BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(_BACKEND_DIR))

# ============================================================
# 导入 Base + 所有 ORM 模型（让 metadata 注册）
# ============================================================
from core.database import Base  # noqa: E402
from core import models  # noqa: F401, E402  触发所有 ORM 模型注册到 Base.metadata
from core import paths as core_paths  # noqa: E402  runtime 读 DATABASE_PATH（monkeypatch 友好）

# ============================================================
# alembic context 配置
# ============================================================

config = context.config

# DB URL 派生（按代码 if-else 实际优先级，高 → 低）：
# 1. alembic.ini sqlalchemy.url 已配置（外部注入或 ini 写死）→ 直接用，跳过派生
# 2. sqlalchemy.url 空 + alembic -x db_path=<path>（命令行）→ 用 -x（autogenerate / 测试）
# 3. sqlalchemy.url 空 + 无 -x → core.paths.DATABASE_PATH 派生（默认生产路径）
#
# 用 runtime 读 core_paths.DATABASE_PATH（不是 import-time 绑定），让测试
# monkeypatch core.paths.DATABASE_PATH 能影响 env.py 派生 URL。
if not config.get_main_option("sqlalchemy.url"):
    cmd_line_db_path = context.get_x_argument(as_dictionary=True).get("db_path")
    if cmd_line_db_path:
        config.set_main_option("sqlalchemy.url", f"sqlite:///{cmd_line_db_path}")
    else:
        config.set_main_option(
            "sqlalchemy.url",
            f"sqlite:///{core_paths.DATABASE_PATH.as_posix()}",
        )

# 日志配置（disable_existing_loggers=False 避免禁用已存在的应用 logger）
if config.config_file_name is not None:
    fileConfig(config.config_file_name, disable_existing_loggers=False)

target_metadata = Base.metadata


def run_migrations_offline() -> None:
    """Offline 模式：生成 SQL 不连接 DB（alembic upgrade head --sql）。"""
    url = config.get_main_option("sqlalchemy.url")
    context.configure(
        url=url,
        target_metadata=target_metadata,
        literal_binds=True,
        dialect_opts={"paramstyle": "named"},
        compare_type=True,
        compare_server_default=True,
        render_as_batch=True,  # SQLite 必须
    )
    with context.begin_transaction():
        context.run_migrations()


def run_migrations_online() -> None:
    """Online 模式：连接 DB 执行 migration（默认路径）。"""
    connectable = engine_from_config(
        config.get_section(config.config_ini_section, {}),
        prefix="sqlalchemy.",
        poolclass=pool.NullPool,
    )
    with connectable.connect() as connection:
        # SQLite 强制 FK（跟 core.database._enable_sqlite_pragmas 一致）
        if connection.dialect.name == "sqlite":
            from sqlalchemy import text

            connection.execute(text("PRAGMA foreign_keys=ON"))

        context.configure(
            connection=connection,
            target_metadata=target_metadata,
            compare_type=True,
            compare_server_default=True,
            render_as_batch=True,  # SQLite 必须（不支持 ALTER COLUMN）
        )
        with context.begin_transaction():
            context.run_migrations()
        # 实测 alembic 1.18 + SQLAlchemy 2.0 在 SQLite 上需要显式 commit DML，
        # 否则 alembic_version INSERT / data migration UPDATE 不持久化（建表 DDL
        # 因为 SQLite auto-commit 能保留，但 DML 被回滚）。
        connection.commit()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
