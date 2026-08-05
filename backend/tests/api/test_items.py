"""API 层测试示例 — 覆盖 items CRUD 全链路（FastAPI TestClient）。

演示 api 层测试模式：用 api_client fixture（httpx ASGITransport），
session 依赖替换为测试 in-memory DB。

业务工程起步时，参照此模式写业务路由测试。
"""

import pytest

pytestmark = pytest.mark.api


class TestListItems:
    """GET /api/items"""

    async def test_list_empty(self, api_client):
        """空 DB 返回空列表。"""
        response = await api_client.get("/api/items")
        assert response.status_code == 200
        body = response.json()
        assert body["success"] is True
        assert body["data"] == []

    async def test_list_returns_items(self, api_client, create_item):
        """有数据时返回 items。"""
        await create_item(name="项目A")
        await create_item(name="项目B")
        response = await api_client.get("/api/items")
        assert response.status_code == 200
        data = response.json()["data"]
        assert len(data) == 2


class TestCreateItem:
    """POST /api/items"""

    async def test_create_success(self, api_client):
        response = await api_client.post(
            "/api/items", json={"name": "新项目", "description": "描述"}
        )
        assert response.status_code == 200
        data = response.json()["data"]
        assert data["name"] == "新项目"
        assert data["description"] == "描述"
        assert "id" in data

    async def test_create_empty_name_rejected(self, api_client):
        """Pydantic min_length=1 校验拦截空 name。"""
        response = await api_client.post("/api/items", json={"name": ""})
        assert response.status_code == 422


class TestGetItem:
    """GET /api/items/{item_id}"""

    async def test_get_existing(self, api_client, create_item):
        item = await create_item(name="存在的项")
        response = await api_client.get(f"/api/items/{item.id}")
        assert response.status_code == 200
        assert response.json()["data"]["name"] == "存在的项"

    async def test_get_nonexistent_returns_not_found(self, api_client):
        """不存在的 item 返回 NOT_FOUND 错误码。"""
        response = await api_client.get("/api/items/nonexistent-id")
        assert response.status_code == 404
        body = response.json()
        assert body["success"] is False
        assert body["error"]["code"] == "NOT_FOUND"


class TestUpdateItem:
    """PATCH /api/items/{item_id}"""

    async def test_update_name(self, api_client, create_item):
        item = await create_item(name="原名")
        response = await api_client.patch(f"/api/items/{item.id}", json={"name": "新名"})
        assert response.status_code == 200
        assert response.json()["data"]["name"] == "新名"


class TestDeleteItem:
    """DELETE /api/items/{item_id}"""

    async def test_delete_success(self, api_client, create_item):
        item = await create_item(name="待删除")
        response = await api_client.delete(f"/api/items/{item.id}")
        assert response.status_code == 200
        # 验证已删除
        list_resp = await api_client.get("/api/items")
        assert list_resp.json()["data"] == []
