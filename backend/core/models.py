"""
ORM 模型

使用 SQLAlchemy 2.0 async 风格（Mapped, mapped_column, AsyncSession）。
所有模型继承自 core.database.Base。

业务工程起步时，在此追加业务表模型（保留 Item 作为最小示例，或替换为业务实体）。
"""

from __future__ import annotations

from datetime import datetime, timezone

from sqlalchemy import DateTime, String, Text
from sqlalchemy.orm import Mapped, mapped_column
from sqlalchemy.types import TypeDecorator

from core import crypto
from core.database import Base

__all__ = ["Base", "EncryptedText", "Item"]


# ============================================================
# 工具函数：统一时间戳
# ============================================================


def _now() -> datetime:
    return datetime.now(timezone.utc)


# ============================================================
# EncryptedText — 透明加解密的 Text 列类型（回流自 ai-audiobook）
# ============================================================


class EncryptedText(TypeDecorator):
    """透明加解密的 Text 列类型（Fernet 对称加密）。

    回流自 ai-audiobook 工程（v0.1.2），通用资产——任何需要存敏感凭据的应用可用。

    - 写入（process_bind_param）：明文 → Fernet 密文
    - 读取（process_result_value）：密文 → 明文

    impl=Text 不改 DB schema（存量列无需 alter），上层 service/API 全部读明文零改动。
    空串透传（本地无需 key 的服务走空串）。加解密实现见 core/crypto.py。
    """

    impl = Text
    cache_ok = True

    def process_bind_param(self, value, dialect):  # type: ignore[override]
        if value is None:
            return value
        return crypto.encrypt(value)

    def process_result_value(self, value, dialect):  # type: ignore[override]
        if value is None:
            return value
        return crypto.decrypt(value)


# ============================================================
# Item — 示例实体（演示 api→service→model 全链路）
# ============================================================


class Item(Base):
    """示例实体：演示最小 CRUD 全链路。

    业务工程起步时，可保留它作为模板示例，或替换为业务实体（如 Project / Image / Task）。
    """

    __tablename__ = "items"

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False, index=True)
    description: Mapped[str | None] = mapped_column(Text, default=None, nullable=True)
    # 示例：加密字段（回流自 ai-audiobook）。业务工程存第三方凭据时用 EncryptedText，
    # DB 存密文、ORM 读明文。空串 default 适配本地无需 key 的服务。需要时取消注释。
    # api_key: Mapped[str] = mapped_column(EncryptedText, default="", nullable=False)

    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), default=_now, nullable=False
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True),
        default=_now,
        onupdate=_now,
        nullable=False,
    )

    def serialize(self) -> dict[str, str | None]:
        """序列化为 API 响应 dict（ISO8601 时间戳）。

        业务工程可改为 Pydantic 响应模型，这里保持轻量 dict 示例。
        """
        return {
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }
