"""集成测试示例 — service 层 + 真实 in-memory DB。

演示 service 层测试模式：用 db_session fixture，直接调 service。

业务工程起步时，参照此模式写业务 service 测试。
"""

import pytest

from core.errors import BadRequestError, NotFoundError
from services.item_service import ItemService

pytestmark = pytest.mark.integration


class TestItemService:
    """ItemService 业务逻辑测试。"""

    async def test_create_and_get(self, db_session):
        svc = ItemService(db_session)
        item = await svc.create_item(name="测试", description="desc")
        assert item.name == "测试"

        fetched = await svc.get_item(item.id)
        assert fetched.id == item.id

    async def test_create_empty_name_raises(self, db_session):
        svc = ItemService(db_session)
        with pytest.raises(BadRequestError):
            await svc.create_item(name="")

    async def test_get_nonexistent_raises(self, db_session):
        svc = ItemService(db_session)
        with pytest.raises(NotFoundError):
            await svc.get_item("nonexistent")

    async def test_update_partial(self, db_session):
        svc = ItemService(db_session)
        item = await svc.create_item(name="原名", description="原描述")
        updated = await svc.update_item(item.id, name="新名")
        assert updated.name == "新名"
        # 未传的字段不变
        assert updated.description == "原描述"

    async def test_delete(self, db_session):
        svc = ItemService(db_session)
        item = await svc.create_item(name="待删")
        await svc.delete_item(item.id)
        with pytest.raises(NotFoundError):
            await svc.get_item(item.id)

    async def test_list_orders_by_created_at_desc(self, db_session):
        svc = ItemService(db_session)
        first = await svc.create_item(name="第一个")
        second = await svc.create_item(name="第二个")
        items = await svc.list_items()
        # 倒序：第二个在前
        assert items[0].id == second.id
        assert items[1].id == first.id
