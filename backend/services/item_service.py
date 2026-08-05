"""
Item service — 示例业务逻辑层

演示 service 层模式：接收 AsyncSession，封装业务逻辑 + 抛 AppError。
业务工程起步时，参照此模式写业务 service（如 image_service / task_service）。
"""

from __future__ import annotations

import uuid

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from core.errors import BadRequestError, NotFoundError
from core.models import Item


class ItemService:
    """示例 service：封装 Item CRUD 业务逻辑。"""

    def __init__(self, session: AsyncSession) -> None:
        self._session = session

    async def list_items(self) -> list[Item]:
        """列出全部 items（按创建时间倒序）。"""
        stmt = select(Item).order_by(Item.created_at.desc())
        result = await self._session.execute(stmt)
        return list(result.scalars().all())

    async def get_item(self, item_id: str) -> Item:
        """取单个 item，不存在抛 NotFoundError。"""
        item = await self._session.get(Item, item_id)
        if item is None:
            raise NotFoundError("Item", item_id)
        return item

    async def create_item(self, name: str, description: str | None = None) -> Item:
        """创建 item。name 为空抛 BadRequestError。"""
        if not name or not name.strip():
            raise BadRequestError("name 不能为空")
        item = Item(
            id=str(uuid.uuid4()),
            name=name.strip(),
            description=description,
        )
        self._session.add(item)
        await self._session.flush()
        return item

    async def update_item(
        self,
        item_id: str,
        name: str | None = None,
        description: str | None = None,
    ) -> Item:
        """部分更新 item（仅更新非 None 字段）。"""
        item = await self.get_item(item_id)
        if name is not None:
            if not name.strip():
                raise BadRequestError("name 不能为空")
            item.name = name.strip()
        if description is not None:
            item.description = description
        await self._session.flush()
        return item

    async def delete_item(self, item_id: str) -> None:
        """删除 item，不存在抛 NotFoundError。"""
        item = await self.get_item(item_id)
        await self._session.delete(item)
        await self._session.flush()
