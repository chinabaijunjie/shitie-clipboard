"""
统一日志

- 控制台 + 文件双输出
- 文件按日期滚动，存放在 <user_data_dir>/logs/
- 用标准 logging 保持零依赖
"""

from __future__ import annotations

import logging
import sys
from logging.handlers import TimedRotatingFileHandler
from pathlib import Path

from core.paths import LOGS_DIR, ensure_user_dirs

_LOGGER_NAME: str = "sidecar"
_INITIALIZED: bool = False


def setup_logging(level: int = logging.INFO) -> logging.Logger:
    """初始化日志系统。幂等，多次调用安全。"""
    global _INITIALIZED
    if _INITIALIZED:
        return logging.getLogger(_LOGGER_NAME)

    ensure_user_dirs()
    log_file: Path = LOGS_DIR / "sidecar.log"

    logger = logging.getLogger(_LOGGER_NAME)
    logger.setLevel(level)
    logger.propagate = False

    formatter = logging.Formatter(
        fmt="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    # 控制台输出（stderr，避免污染 FastAPI 响应）
    console = logging.StreamHandler(sys.stderr)
    console.setLevel(level)
    console.setFormatter(formatter)
    logger.addHandler(console)

    # 文件输出（按天滚动，保留 14 天）
    file_handler = TimedRotatingFileHandler(
        filename=log_file,
        when="midnight",
        backupCount=14,
        encoding="utf-8",
    )
    file_handler.setLevel(level)
    file_handler.setFormatter(formatter)
    logger.addHandler(file_handler)

    _INITIALIZED = True
    logger.info("日志系统初始化完成 → %s", log_file)
    return logger


def get_logger(name: str | None = None) -> logging.Logger:
    """获取子 logger。未初始化时自动 fallback 到 root。"""
    if name is None or name == _LOGGER_NAME:
        return logging.getLogger(_LOGGER_NAME)
    return logging.getLogger(_LOGGER_NAME).getChild(name)
