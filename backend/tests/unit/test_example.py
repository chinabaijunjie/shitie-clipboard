"""单元测试示例 — 纯函数，无 DB 无 IO。

业务工程起步时，参照此结构写纯函数单测（如算法 / 工具函数）。
"""

from core.errors import ErrorCode, http_status_for


class TestErrorCodeMapping:
    """错误码 → HTTP 状态码映射单测。"""

    def test_not_found_maps_to_404(self):
        assert http_status_for(ErrorCode.NOT_FOUND) == 404

    def test_bad_request_maps_to_400(self):
        assert http_status_for(ErrorCode.BAD_REQUEST) == 400

    def test_conflict_maps_to_409(self):
        assert http_status_for(ErrorCode.CONFLICT) == 409

    def test_unknown_code_falls_back_to_500(self):
        assert http_status_for("NONEXISTENT_CODE") == 500
