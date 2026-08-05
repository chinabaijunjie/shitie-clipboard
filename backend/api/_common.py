"""
API 路由层公共工具

- ApiResponse 统一封装（ok / fail）
- Session 依赖注入简写
"""

from __future__ import annotations

from typing import Any

from fastapi import Depends

from core.database import get_session

# ============================================================
# 统一响应
# ============================================================


def ok(data: Any = None) -> dict[str, Any]:
    """成功响应。"""
    return {"success": True, "data": data, "error": None}


def fail(error: dict[str, Any]) -> dict[str, Any]:
    """失败响应（一般由异常处理器自动包装）。"""
    return {"success": False, "data": None, "error": error}


# ============================================================
# Session 依赖简写
# ============================================================

SessionDep = Depends(get_session)
