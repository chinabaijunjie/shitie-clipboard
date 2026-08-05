#!/bin/bash
# build-sidecar.sh — 用 PyInstaller 打包 sidecar binary（最简单 onefile）
#
# 用法：
#   chmod +x scripts/build-sidecar.sh
#   ./scripts/build-sidecar.sh
#
# 产出：
#   resources/bin/sidecar  — 单文件可执行（FastAPI sidecar，onefile 模式）
#
# 打包后 tauri.conf.json 的 bundle.resources glob（resources/bin/）自动收进 .app。
# Rust 端 sidecar.rs find_python_runner 探测 resources/bin/sidecar（生产模式）。
#
# 前置：
#   - Python 环境已装 pyinstaller（pip install pyinstaller）
#   - Python 环境已装 backend/requirements.txt 依赖（fastapi/uvicorn/sqlalchemy 等）
#
# 环境变量：
#   PYTHON  — 指定 Python 解释器（默认用 conda activate 后的 python）
#   例如：PYTHON=/path/to/conda/envs/xxx/bin/python ./scripts/build-sidecar.sh

set -e
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BACKEND_DIR="${PROJECT_ROOT}/backend"
SPEC_FILE="${BACKEND_DIR}/build/sidecar.spec"
TARGET_DIR="${PROJECT_ROOT}/resources/bin"

echo "🏗️  Clipboard Manager — PyInstaller sidecar 打包（onefile）"
echo "项目根：${PROJECT_ROOT}"
echo "产物目录：${TARGET_DIR}"
echo ""

# 确定 Python 解释器
if [ -n "${PYTHON:-}" ]; then
    PY_BIN="${PYTHON}"
elif command -v python >/dev/null 2>&1; then
    PY_BIN="$(command -v python)"
else
    echo "❌ 找不到 python。请先激活 Python 环境（conda activate / venv），或设 PYTHON 环境变量。"
    exit 1
fi

# 校验 pyinstaller 已装
if ! "${PY_BIN}" -c "import PyInstaller" >/dev/null 2>&1; then
    echo "❌ pyinstaller 未安装。请在当前 Python 环境执行：pip install pyinstaller"
    echo "   当前 Python：${PY_BIN}"
    exit 1
fi

echo "Python：${PY_BIN}"
echo "PyInstaller：$("${PY_BIN}" -c 'import PyInstaller; print(PyInstaller.__version__)')"
echo ""

mkdir -p "${TARGET_DIR}"

# ============================================================
# PyInstaller 打包（在项目根运行，spec 内用 SPECPATH 相对定位）
# ============================================================
echo "📦 开始打包…"
"${PY_BIN}" -m PyInstaller "${SPEC_FILE}" \
    --noconfirm \
    --distpath "${TARGET_DIR}" \
    --workpath "${PROJECT_ROOT}/backend/build/_pyinstaller_build"
echo "✅ PyInstaller 打包完成"
echo ""

# ============================================================
# 验证产物
# ============================================================
BINARY="${TARGET_DIR}/sidecar"
if [ ! -f "${BINARY}" ]; then
    echo "❌ 产物不存在：${BINARY}"
    echo "   检查 PyInstaller 输出（上方日志）是否有错误。"
    exit 1
fi

FILE_SIZE=$(du -h "${BINARY}" | cut -f1)
echo "✅ 产物就绪：${BINARY}（${FILE_SIZE}）"
echo ""
echo "下一步：npm run tauri dev（开发模式用 SIDECAR_DEV_PYTHON，不走 binary）"
echo "       或 npm run tauri build（生产打包，binary 随 .app 分发）"
