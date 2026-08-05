"""
统一配置管理

- 应用元信息：名称、版本、端口
- @dataclass(frozen=True) + lru_cache 配置单例
- 业务工程起步时，在此追加业务配置（如 ModelConfig / EngineConfig）
"""

from __future__ import annotations

from dataclasses import dataclass, field
from functools import lru_cache

# ============================================================
# 应用元信息
# ============================================================

APP_NAME: str = "Clipboard Manager"
APP_VERSION: str = "1.0.0"

# Python sidecar 监听端口（与前端 core/constants.ts SIDECAR_PORT 保持一致）
# ⚠️ 三处对齐：Rust sidecar.rs SIDECAR_PORT + 本处 + TS constants.ts
SIDECAR_HOST: str = "127.0.0.1"
SIDECAR_PORT: int = 9880


@dataclass(frozen=True)
class SidecarConfig:
    host: str = SIDECAR_HOST
    port: int = SIDECAR_PORT
    # 健康检查超时（秒）
    health_check_timeout: int = 30
    health_check_interval: float = 0.5


@dataclass(frozen=True)
class AppConfig:
    app_name: str = APP_NAME
    app_version: str = APP_VERSION
    sidecar: SidecarConfig = field(default_factory=SidecarConfig)
    # 业务工程起步时，在此追加业务配置字段（如 model / engine / export）


@lru_cache(maxsize=1)
def get_config() -> AppConfig:
    """全局配置单例。可改为从 YAML/JSON 加载用户覆盖。"""
    return AppConfig()
