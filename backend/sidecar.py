"""
Clipboard Manager — Python FastAPI Sidecar 入口

启动方式：
    # 开发期（tauri dev 会通过 SIDECAR_DEV_PYTHON 自动 spawn，也可手动跑）
    python backend/sidecar.py

端口：9880（与前端 core/constants.ts SIDECAR_PORT + Rust sidecar.rs SIDECAR_PORT 保持一致）

本入口是模板骨架：health/status 核心端点 + DB init 框架 + SSE 事件总线 + graceful shutdown。
业务工程起步时，在 lifespan 追加业务初始化（引擎预热/任务恢复等），在底部追加 include_router。
"""

from __future__ import annotations

import asyncio
import json
import os
import sys
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

# 把 backend/ 目录加入 sys.path，便于 `from core.xxx import` 写法
BACKEND_DIR = Path(__file__).resolve().parent
if str(BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(BACKEND_DIR))

# 把项目根目录也加入（部分模块需要相对路径定位 resources/）
APP_ROOT = BACKEND_DIR.parent
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))


# ============================================================
# .env 加载（必须在 core.config / core.paths import 之前）
# ============================================================


def _load_dotenv_files() -> None:
    """简单 .env 加载器（无 python-dotenv 依赖）。

    按优先级加载：APP_ROOT/.env.local > APP_ROOT/.env > BACKEND_DIR/.env。
    仅解析 KEY=VALUE 行，忽略 # 注释；已存在的环境变量不覆盖。
    """
    candidates = [
        APP_ROOT / ".env.local",
        APP_ROOT / ".env",
        BACKEND_DIR / ".env",
    ]
    for env_path in candidates:
        if not env_path.is_file():
            continue
        try:
            for raw_line in env_path.read_text(encoding="utf-8").splitlines():
                line = raw_line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                key, _, value = line.partition("=")
                key = key.strip()
                value = value.strip().strip('"').strip("'")
                if key and key not in os.environ:
                    os.environ[key] = value
        except OSError:
            # 读 .env 失败不阻塞启动，让后续逻辑抛清晰错误
            continue


_load_dotenv_files()


from fastapi import FastAPI, Request  # noqa: E402  必须在 sys.path 设置后
from fastapi.middleware.cors import CORSMiddleware  # noqa: E402
from fastapi.responses import JSONResponse, StreamingResponse  # noqa: E402

from core.config import APP_NAME, APP_VERSION, get_config  # noqa: E402
from core.database import (  # noqa: E402
    dispose_engine,
    init_db,
    init_engine,
)
from core.errors import AppError  # noqa: E402
from core.events import event_bus  # noqa: E402
from core.logger import setup_logging  # noqa: E402
from core.paths import ensure_user_dirs  # noqa: E402

# ============================================================
# 应用生命周期
# ============================================================


@asynccontextmanager
async def lifespan(app: FastAPI):
    """应用启动/关闭钩子。

    业务工程起步时，在此追加业务初始化（引擎预热 / 任务恢复 / 预置数据等）。
    """
    logger = setup_logging()
    logger.info("%s v%s 启动中…", APP_NAME, APP_VERSION)

    ensure_user_dirs()
    init_engine()
    await init_db()
    logger.info("数据库初始化完成")

    yield

    logger.info("应用关闭，释放数据库连接…")
    await dispose_engine()
    logger.info("已关闭")


# ============================================================
# FastAPI 应用
# ============================================================

config = get_config()

app = FastAPI(
    title=APP_NAME,
    version=APP_VERSION,
    lifespan=lifespan,
    # 默认关闭 swagger，调试时设 SIDECAR_DEBUG=1 开启 /docs
    docs_url="/docs" if os.environ.get("SIDECAR_DEBUG") else None,
    redoc_url=None,
)

# CORS：Tauri 前端 origin 收窄清单
# - macOS/Linux 生产：tauri://localhost（Tauri 2.x 默认）
# - Windows 生产：https://tauri.localhost（Tauri 2.x Windows 自定义协议）
# - 开发模式：http://localhost:5173（tauri.conf.json:devUrl，Vite dev server）
# 不使用 ["*"] 避免任何本地进程（含恶意脚本）跨域调 sidecar API。
ALLOWED_ORIGINS: list[str] = [
    "tauri://localhost",
    "https://tauri.localhost",
    "http://localhost:5173",
]
app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_credentials=False,
    allow_methods=["*"],
    allow_headers=["*"],
)


# ============================================================
# 统一异常 → ApiResponse 包装
# ============================================================


@app.exception_handler(AppError)
async def app_error_handler(request: Request, exc: AppError) -> JSONResponse:
    return JSONResponse(
        status_code=exc.http_status,
        content={
            "success": False,
            "data": None,
            "error": exc.to_dict(),
        },
    )


# ============================================================
# 核心端点：health / status
# ============================================================


@app.get("/api/health")
async def health() -> dict[str, Any]:
    """健康检查 — Rust sidecar 启动时轮询此端点。"""
    return {
        "success": True,
        "data": {
            "status": "ok",
            "app": APP_NAME,
            "version": APP_VERSION,
            "pid": os.getpid(),
        },
        "error": None,
    }


@app.get("/api/status")
async def status() -> dict[str, Any]:
    """应用状态 — 版本、数据目录等元信息。"""
    from core.paths import get_user_data_dir

    return {
        "success": True,
        "data": {
            "app": APP_NAME,
            "version": APP_VERSION,
            "data_dir": str(get_user_data_dir()),
        },
        "error": None,
    }


# ============================================================
# SSE 进度推送
# ============================================================

# 心跳间隔：长时间无事件时发 SSE comment（: heartbeat）防反向代理 idle timeout。
SSE_HEARTBEAT_SEC: int = 15


@app.get("/api/events/{channel}")
async def sse_events(channel: str) -> StreamingResponse:
    """SSE 长连接 — 推送任务进度等事件。

    频道名约定由业务层定义（core/events.py Channels + channel_for_* helper）。

    客户端断开（关 tab / 切页 / 网络断）由 Starlette 取消 async generator
    触发 GeneratorExit 进 finally，event_bus.unsubscribe 必跑防内存泄漏。

    15s 心跳：长时间无事件时防代理 idle timeout 关连接。
    """

    async def event_stream() -> Any:
        queue = await event_bus.subscribe(channel)
        try:
            # 立即 yield 连接建立标记 — SSE comment 行客户端忽略，
            # 但让握手能立即返回（否则会等第一个 data 或 15s 心跳才完成）
            yield ": connected\n\n"
            while True:
                try:
                    event = await asyncio.wait_for(queue.get(), timeout=SSE_HEARTBEAT_SEC)
                except asyncio.TimeoutError:
                    # SSE comment line：客户端忽略，仅保活
                    yield ": heartbeat\n\n"
                    continue
                payload = {
                    "type": event.type,
                    "data": event.data,
                    "timestamp": event.timestamp,
                }
                yield f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"
        finally:
            await event_bus.unsubscribe(channel, queue)

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            # 防 nginx 缓冲（本地 sidecar 无效但不破）
            "X-Accel-Buffering": "no",
        },
    )


# ============================================================
# 挂载业务路由（业务工程起步时在此追加 include_router）
# ============================================================
from api import items as items_api  # noqa: E402

app.include_router(items_api.router)


# ============================================================
# 入口
# ============================================================


def main() -> None:
    """直接 python sidecar.py 启动。"""
    import uvicorn

    logger = setup_logging()
    logger.info("启动 uvicorn — host=%s port=%d", config.sidecar.host, config.sidecar.port)
    uvicorn.run(
        # 直接传 app 对象，不用字符串 "sidecar:app"。
        # 字符串导入在 PyInstaller onefile 模式下会失败（Could not import module "sidecar"）：
        # onefile 把模块打进 _MEIPASS，uvicorn 的字符串导入走标准 import 机制找不到。
        # 直接传对象绕过此问题，frozen/开发双模式都工作。
        app,
        host=config.sidecar.host,
        port=config.sidecar.port,
        reload=False,
        log_level="info",
        access_log=False,
        # 优雅关闭超时（秒）：收到 SIGTERM/SIGINT 后，最多等 5s 让在途请求
        # 完成，超时则强制断开残留连接立即退出。
        # 不配则默认 None（无限等待）→ SSE 长连接永远不关 →
        # sidecar 卡在 "Waiting for connections to close" 永久不退出。
        timeout_graceful_shutdown=5,
    )


if __name__ == "__main__":
    main()
