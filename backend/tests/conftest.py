"""顶层 conftest — sys.path 注入 + 测试环境变量 + 通用 fixtures。

组织结构：
- §0 sys.path + 环境变量（必须最先执行）
- §1 database: in-memory SQLite + StaticPool，每测试函数新建 session
- §2 factories: build_item / create_item（示例实体）
- §3 api_client: FastAPI 异步测试客户端（session 依赖替换为测试 db_session）

业务工程起步时，参照此结构追加业务 factory / mock。
"""

from __future__ import annotations

# ============================================================
# §0 sys.path + 测试环境变量（必须最先执行，在任何 backend import 之前）
# ============================================================
import os
import sys
from pathlib import Path

# 把 backend/ 加入 sys.path，让 `from services.xxx import` / `from core.xxx import` 可用
BACKEND_DIR = Path(__file__).parent.parent
if str(BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(BACKEND_DIR))

# 测试环境变量（避免读到生产配置 + 数据目录隔离）
os.environ.setdefault("SIDECAR_DATA_DIR", "/tmp/tauri-template-test")
os.environ.setdefault("PYTHONPATH", str(BACKEND_DIR))


# ============================================================
# §1 database 部分
# ============================================================

import uuid
from collections.abc import AsyncIterator
from typing import Any

import pytest_asyncio
from sqlalchemy import event
from sqlalchemy.ext.asyncio import (
    AsyncSession,
    async_sessionmaker,
    create_async_engine,
)
from sqlalchemy.pool import StaticPool

from core.database import _enable_sqlite_pragmas
from core.models import Base, Item


@pytest_asyncio.fixture
async def db_engine() -> AsyncIterator:
    """每测试函数新建 in-memory engine。

    StaticPool 保证 :memory: 在同一连接内共享（SQLite 特性）。
    防御：先 dispose 任何已存在的全局 engine，避免 core.database._engine 残留污染。

    复用 core.database._enable_sqlite_pragmas（FK + WAL + busy_timeout），
    让 PRAGMA 在测试中实际生效（与生产一致）。
    in-memory DB 下 WAL 静默回退为 "memory"，busy_timeout 无害生效。
    """
    from core import database

    if database._engine is not None:
        await database.dispose_engine()

    engine = create_async_engine(
        "sqlite+aiosqlite:///:memory:",
        poolclass=StaticPool,
        connect_args={"check_same_thread": False},
    )
    event.listens_for(engine.sync_engine, "connect")(_enable_sqlite_pragmas)

    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    try:
        yield engine
    finally:
        async with engine.begin() as conn:
            await conn.run_sync(Base.metadata.drop_all)
        await engine.dispose()


@pytest_asyncio.fixture
async def db_session(db_engine) -> AsyncIterator[AsyncSession]:
    """每测试函数独立 session，结束后自动 rollback + 关闭。

    用法：
        async def test_xxx(db_session):
            svc = ItemService(db_session)
            ...
    """
    session_maker = async_sessionmaker(db_engine, expire_on_commit=False, class_=AsyncSession)
    async with session_maker() as session:
        try:
            yield session
            await session.rollback()
        finally:
            await session.close()


# ============================================================
# §2 factories 部分（示例实体 Item）
# ============================================================


def build_item(**overrides: Any) -> Item:
    """构建 Item 实例（未保存）。"""
    defaults = {
        "id": str(uuid.uuid4()),
        "name": f"测试项_{uuid.uuid4().hex[:8]}",
        "description": None,
    }
    defaults.update(overrides)
    return Item(**defaults)


@pytest_asyncio.fixture
async def create_item(db_session):
    """异步 create fixture（build + add + flush + return）。"""

    async def _create(**overrides):
        item = build_item(**overrides)
        db_session.add(item)
        await db_session.flush()
        return item

    return _create


# ============================================================
# §3 api_client（httpx.AsyncClient + ASGITransport）
# ============================================================


@pytest_asyncio.fixture
async def api_client(db_session):
    """FastAPI 异步测试客户端，session 依赖替换为测试 db_session。

    用 ASGITransport 而非 TestClient，避免触发 sidecar.app lifespan（会初始化
    真实 DB 等资源）。用 dependency_overrides 替换 get_session，与 db_session
    fixture 共享同一 session。

    用法：
        async def test_list_empty(api_client):
            response = await api_client.get("/api/items")
            assert response.status_code == 200
            assert response.json() == {"success": True, "data": [], "error": None}
    """
    from httpx import ASGITransport, AsyncClient

    from core.database import get_session
    from sidecar import app

    async def _override_session():
        yield db_session

    app.dependency_overrides[get_session] = _override_session
    try:
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            yield client
    finally:
        app.dependency_overrides.clear()
