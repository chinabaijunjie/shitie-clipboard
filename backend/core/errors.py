"""
统一错误码 + 业务异常

- 所有 service 层抛业务异常都从这里出
- API 层统一转 HTTP 状态码 + ApiResponse 包装
- 错误码字符串与前端 core/api/types.ts ApiErrorCode 保持一致（镜像契约）

⚠️ 前后端错误码必须镜像：本文件的 ErrorCode 与 src/core/api/types.ts 的 ApiErrorCode
   是同一份契约的两端实现，任一端新增/删除错误码必须同步另一端。
   业务工程起步时，在两端同步追加业务错误码（如 IMAGE_GEN_FAILED / API_KEY_INVALID）。
"""

from __future__ import annotations

from typing import Any

# ============================================================
# 错误码常量（通用错误码，业务错误码在业务工程追加）
# ============================================================


class ErrorCode:
    # 通用
    UNKNOWN = "UNKNOWN"
    BAD_REQUEST = "BAD_REQUEST"
    UNAUTHORIZED = "UNAUTHORIZED"
    FORBIDDEN = "FORBIDDEN"
    NOT_FOUND = "NOT_FOUND"
    CONFLICT = "CONFLICT"
    INTERNAL_ERROR = "INTERNAL_ERROR"
    SERVICE_UNAVAILABLE = "SERVICE_UNAVAILABLE"

    # 基础设施
    SIDECAR_NOT_READY = "SIDECAR_NOT_READY"


# 错误码 → HTTP 状态码映射
_CODE_TO_HTTP: dict[str, int] = {
    ErrorCode.UNKNOWN: 500,
    ErrorCode.BAD_REQUEST: 400,
    ErrorCode.UNAUTHORIZED: 401,
    ErrorCode.FORBIDDEN: 403,
    ErrorCode.NOT_FOUND: 404,
    ErrorCode.CONFLICT: 409,
    ErrorCode.INTERNAL_ERROR: 500,
    ErrorCode.SERVICE_UNAVAILABLE: 503,
    ErrorCode.SIDECAR_NOT_READY: 503,
}


def http_status_for(code: str) -> int:
    return _CODE_TO_HTTP.get(code, 500)


# ============================================================
# 业务异常基类
# ============================================================


class AppError(Exception):
    """业务异常基类。API 层捕获后转 ApiResponse。"""

    code: str = ErrorCode.UNKNOWN
    http_status: int = 500

    def __init__(
        self,
        message: str,
        *,
        code: str | None = None,
        detail: Any | None = None,
        http_status: int | None = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.code = code or self.code
        self.detail = detail
        self.http_status = http_status or http_status_for(self.code)

    def to_dict(self) -> dict[str, Any]:
        # dict / list detail 原样保留（前端 JSON 序列化 + 程序化访问字段），
        # 其他类型（str/int/...）转 str（兼容老行为，避免非 JSON 安全类型进响应）。
        detail: Any = self.detail
        if detail is not None and not isinstance(detail, (dict, list)):
            detail = str(detail)
        return {
            "code": self.code,
            "message": self.message,
            "detail": detail,
        }


# ============================================================
# 常见子类（通用）
# ============================================================


class NotFoundError(AppError):
    code = ErrorCode.NOT_FOUND

    def __init__(self, resource: str, resource_id: str | None = None) -> None:
        msg = f"{resource} 不存在" + (f"：{resource_id}" if resource_id else "")
        super().__init__(msg)


class BadRequestError(AppError):
    code = ErrorCode.BAD_REQUEST


class ConflictError(AppError):
    code = ErrorCode.CONFLICT


class ServiceUnavailableError(AppError):
    """服务暂不可用（HTTP 503）—— 功能存在但当前不可用。

    跟 BadRequestError (400) 区分：
    BadRequestError 是客户端错误（请求格式错），503 是服务端能力问题。
    """

    code = ErrorCode.SERVICE_UNAVAILABLE
