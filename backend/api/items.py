"""
Items API — 示例 CRUD 路由

演示 api→service→model 全链路模式：
- 路由层只做请求解析 + 调 service + 包装响应
- 业务逻辑在 service 层
- 异常由 sidecar.py 全局 handler 统一转 ApiResponse

业务工程起步时，参照此模式写业务路由（如 images.py / tasks.py）。
"""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter
from pydantic import BaseModel, Field
from sqlalchemy.ext.asyncio import AsyncSession

from api._common import SessionDep, ok
from services.item_service import ItemService

router = APIRouter(prefix="/api/items", tags=["items"])


# ============================================================
# 请求模型（Pydantic，输入校验在系统边界）
# ============================================================


class CreateItemRequest(BaseModel):
    name: str = Field(..., min_length=1, max_length=255)
    description: str | None = None


class UpdateItemRequest(BaseModel):
    name: str | None = Field(default=None, min_length=1, max_length=255)
    description: str | None = None


# ============================================================
# 路由
# ============================================================


@router.get("")
async def list_items(session: AsyncSession = SessionDep) -> dict[str, Any]:
    """列出全部 items。"""
    svc = ItemService(session)
    items = await svc.list_items()
    return ok([item.serialize() for item in items])


@router.post("")
async def create_item(
    payload: CreateItemRequest, session: AsyncSession = SessionDep
) -> dict[str, Any]:
    """创建 item。"""
    svc = ItemService(session)
    item = await svc.create_item(name=payload.name, description=payload.description)
    return ok(item.serialize())


@router.get("/{item_id}")
async def get_item(item_id: str, session: AsyncSession = SessionDep) -> dict[str, Any]:
    """取单个 item。"""
    svc = ItemService(session)
    item = await svc.get_item(item_id)
    return ok(item.serialize())


@router.patch("/{item_id}")
async def update_item(
    item_id: str, payload: UpdateItemRequest, session: AsyncSession = SessionDep
) -> dict[str, Any]:
    """部分更新 item（仅更新非 None 字段）。"""
    svc = ItemService(session)
    item = await svc.update_item(item_id, name=payload.name, description=payload.description)
    return ok(item.serialize())


@router.delete("/{item_id}")
async def delete_item(item_id: str, session: AsyncSession = SessionDep) -> dict[str, Any]:
    """删除 item。"""
    svc = ItemService(session)
    await svc.delete_item(item_id)
    return ok(None)
