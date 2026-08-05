# 测试

## 目录结构

```
tests/
├── conftest.py           顶层 fixtures（sys.path + in-memory DB + api_client + factory）
├── unit/                 单元测试（纯函数，无 DB 无 IO，<1s）
├── integration/          集成测试（service + 真实 in-memory DB，<5s）
└── api/                  API 层测试（FastAPI TestClient，httpx ASGITransport）
```

## 运行

```bash
# 在 backend/ 目录下，用项目的 Python 环境（conda / venv）
cd backend

# 全部测试
python -m pytest

# 按 marker 过滤
python -m pytest -m unit          # 仅单元测试
python -m pytest -m integration   # 仅集成测试
python -m pytest -m api           # 仅 API 测试

# 覆盖率
python -m pytest --cov=. --cov-report=term-missing
```

## 测试基础设施（模板资产）

- **in-memory DB**：`db_engine` / `db_session` fixture，每测试函数新建 in-memory SQLite，
  隔离干净 + 快速。复用 `core.database._enable_sqlite_pragmas`（FK + WAL + busy_timeout），
  与生产 PRAGMA 一致。
- **api_client**：httpx AsyncClient + ASGITransport，session 依赖替换为测试 DB，
  覆盖路由层（错误响应序列化 / 路由顺序 / ApiResponse 包装）。
- **factory**：`build_item`（构建未保存实例）+ `create_item`（build + add + flush）。

业务工程起步时，参照 `test_items.py` / `test_item_service.py` 写业务测试。
