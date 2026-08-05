"""crypto 模块单测（Fernet 凭据加密基础设施）。

回流自 ai-audiobook 工程（v0.1.2），通用资产。

验证：
- encrypt/decrypt 往返一致
- 空串透传（本地无需 key 的服务）
- 同明文两次加密得不同密文（Fernet 随机 IV）
- master key 文件首次生成 + 0600 权限
- master key 持久（二次调用读同一 key）
- 解密失败抛 RuntimeError（不静默吞错）

测试隔离：每个测试 monkeypatch _resolve_user_data_dir 重定向到 tmp_path，
并 clear lru_cache，避免污染真实数据目录 + 测试间互不影响。
"""

from __future__ import annotations

import os
import stat
from pathlib import Path

import pytest

from core import crypto


@pytest.fixture(autouse=True)
def _isolated_master_key(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """每个测试用独立 tmp 目录 + 清空 lru_cache，避免污染真实数据 + 测试隔离。"""
    monkeypatch.setattr(crypto, "_resolve_user_data_dir", lambda: tmp_path)
    crypto.get_master_key.cache_clear()
    crypto.get_fernet.cache_clear()
    yield
    crypto.get_master_key.cache_clear()
    crypto.get_fernet.cache_clear()


class TestEncryptDecrypt:
    """encrypt/decrypt 基础语义。"""

    def test_roundtrip(self):
        """加密后解密还原明文。"""
        plaintext = "sk-example-abcdef1234567890"
        enc = crypto.encrypt(plaintext)
        assert enc != plaintext
        assert crypto.decrypt(enc) == plaintext

    def test_empty_string_passthrough(self):
        """空串透传不加密（本地无需 key 的服务走空串）。"""
        assert crypto.encrypt("") == ""
        assert crypto.decrypt("") == ""

    def test_same_plaintext_different_ciphertext(self):
        """同明文两次加密得不同密文（Fernet 每次用随机 IV）。"""
        plaintext = "sk-same-key"
        enc1 = crypto.encrypt(plaintext)
        enc2 = crypto.encrypt(plaintext)
        assert enc1 != enc2  # 随机 IV
        # 但都能解密回同一明文
        assert crypto.decrypt(enc1) == plaintext
        assert crypto.decrypt(enc2) == plaintext

    def test_unicode_plaintext(self):
        """非 ASCII 明文（如含中文的 key）往返一致。"""
        plaintext = "密钥-sk-测试-123"
        enc = crypto.encrypt(plaintext)
        assert crypto.decrypt(enc) == plaintext


class TestMasterKeyFile:
    """master key 文件生成与持久化。"""

    def test_first_call_generates_key_file(self, tmp_path: Path):
        """首次调用生成 master.key 文件。"""
        key_path = tmp_path / crypto.MASTER_KEY_FILENAME
        assert not key_path.exists()
        crypto.get_master_key()
        assert key_path.exists()

    def test_key_file_permission_0600(self, tmp_path: Path):
        """master.key 权限为 0600（仅 owner 读写）。"""
        crypto.get_master_key()
        key_path = tmp_path / crypto.MASTER_KEY_FILENAME
        mode = stat.S_IMODE(os.stat(key_path).st_mode)
        assert mode == 0o600, f"权限应为 0600，实际 {oct(mode)}"

    def test_key_persistent_across_calls(self, tmp_path: Path):
        """同一目录二次调用读同一 key（持久化）。"""
        key1 = crypto.get_master_key()
        # 清缓存模拟进程重启，应从文件读到同一 key
        crypto.get_master_key.cache_clear()
        key2 = crypto.get_master_key()
        assert key1 == key2

    def test_fernet_singleton(self):
        """get_fernet 进程内单例（lru_cache）。"""
        f1 = crypto.get_fernet()
        f2 = crypto.get_fernet()
        assert f1 is f2


class TestDecryptFailure:
    """解密失败显式抛错（不静默吞错）。"""

    def test_invalid_ciphertext_raises(self):
        """无效密文抛 RuntimeError（带根因提示）。"""
        with pytest.raises(RuntimeError, match="凭据解密失败"):
            crypto.decrypt("not-a-valid-fernet-token")

    def test_wrong_master_key_raises(self, tmp_path: Path):
        """用 master key A 加密，换 master key B 后解密失败抛错。

        模拟「用户换了数据目录 / master key 丢失重生成」场景。
        """
        enc = crypto.encrypt("sk-original")
        # 换新数据目录 → 新 master key
        crypto.get_master_key.cache_clear()
        crypto.get_fernet.cache_clear()
        new_dir = tmp_path / "new_data_dir"
        new_dir.mkdir()
        # 重新 monkeypatch（_isolated_master_key fixture 只设一次，这里手动改）
        import core.crypto as c

        original_resolve = c._resolve_user_data_dir
        c._resolve_user_data_dir = lambda: new_dir
        try:
            with pytest.raises(RuntimeError, match="凭据解密失败"):
                crypto.decrypt(enc)
        finally:
            c._resolve_user_data_dir = original_resolve
