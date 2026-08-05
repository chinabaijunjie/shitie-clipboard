"""
列级加密基础设施 — 为敏感凭据（如用户配置的第三方 API Key）提供 Fernet 对称加密。

回流自 ai-audiobook 工程（v0.1.2，2026-07-01），通用资产。

设计：
- DB 存密文，ORM 读明文（经 EncryptedText TypeDecorator 透明加解密，见 models.py）
- master key 落 USER_DATA_DIR/master.key（0600 权限，进程级单例）
- 加密链路：master key 文件 → lru_cache → Fernet → encrypt/decrypt helper

设计要点：
- master key 首次启动由本模块用 Fernet.generate_key() 生成并写文件，chmod 0o600
  （SSH key / DB 凭据的主流存储方式；密钥不出文件系统，不进 env / git / bundle）
- 空串透传不加密（本地无需 key 的服务走空串，避免掩码与真值判断错乱）
- 解密失败显式抛 RuntimeError（不静默吞错，给清晰根因：master key 丢失或密文损坏）

适用场景：任何需要存储用户提供的第三方凭据的应用（云端模型 API key、第三方服务 token 等）。
"""

from __future__ import annotations

import logging
import os
from functools import lru_cache

from cryptography.fernet import Fernet, InvalidToken

from core.paths import _resolve_user_data_dir

_LOGGER = logging.getLogger(__name__)

# master key 文件名（落 USER_DATA_DIR 下，与 data.db / config.json 同级）
MASTER_KEY_FILENAME = "master.key"
# 仅 owner 可读写（macOS/Linux 文件权限）
_MASTER_KEY_MODE = 0o600


@lru_cache(maxsize=1)
def get_master_key() -> bytes:
    """获取 Fernet master key（进程级单例，首次调用时按需生成并落盘）。

    - 文件不存在：Fernet.generate_key() 生成 → 写文件 + chmod 0o600
    - 文件存在：直接读返回

    Returns:
        44 字节 urlsafe-base64 编码的 master key（Fernet 格式要求）。

    注意：返回值是密钥本身，**禁止日志打印**。
    """
    key_path = _resolve_user_data_dir() / MASTER_KEY_FILENAME
    if key_path.exists():
        return key_path.read_bytes()
    # 首次生成
    key_path.parent.mkdir(parents=True, exist_ok=True)
    key = Fernet.generate_key()
    # 写入后立即收紧权限（0600：仅 owner 可读写）
    key_path.write_bytes(key)
    os.chmod(key_path, _MASTER_KEY_MODE)
    _LOGGER.info("已生成凭据加密用 master key：%s", key_path)
    return key


@lru_cache(maxsize=1)
def get_fernet() -> Fernet:
    """获取进程级 Fernet 单例（复用 master key）。"""
    return Fernet(get_master_key())


def encrypt(plaintext: str) -> str:
    """加密明文为 Fernet 密文（urlsafe-base64 字符串）。

    空串透传（本地无需 key 的服务走空串，加密空串会让掩码 len() 判断错乱）。
    """
    if not plaintext:
        return plaintext
    return get_fernet().encrypt(plaintext.encode("utf-8")).decode("ascii")


def decrypt(ciphertext: str) -> str:
    """解密 Fernet 密文为明文。

    空串透传。解密失败抛 RuntimeError（不静默吞错）——master key 丢失或密文损坏
    时给清晰根因，避免下游拿到错误值产生迷惑性故障。

    Raises:
        RuntimeError: 密文无法解密（master key 不匹配或密文损坏）。
    """
    if not ciphertext:
        return ciphertext
    try:
        return get_fernet().decrypt(ciphertext.encode("ascii")).decode("utf-8")
    except InvalidToken as e:
        # 不暴露密文片段，只给根因定位信息
        raise RuntimeError(
            "凭据解密失败：master key 可能已变更，或 DB 中密文损坏。"
            "若更换了数据目录，需重配受影响的凭据。"
        ) from e
