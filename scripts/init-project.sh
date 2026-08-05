#!/bin/bash
# init-project.sh — 基于模板创建新工程（复制 + 改名一步到位）
#
# 用法（在模板工程根目录执行）：
#   ./scripts/init-project.sh
#
# 脚本会交互式询问 3 项（新工程路径 / 工程名 / identifier / 显示名），
# 然后：rsync 复制模板（排除 .git/node_modules/target 等）→ sed 批量替换占位符 → git init。
#
# 详见同目录 README.md（脚本说明书）。

set -euo pipefail

# ============================================================
# 颜色输出
# ============================================================
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m' # No Color

info()  { echo -e "${CYAN}ℹ️  $1${NC}"; }
ok()    { echo -e "${GREEN}✅ $1${NC}"; }
warn()  { echo -e "${YELLOW}⚠️  $1${NC}"; }
error() { echo -e "${RED}❌ $1${NC}"; }

# ============================================================
# 前置检查：必须在模板根目录执行
# ============================================================
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEMPLATE_ROOT="$(dirname "$SCRIPT_DIR")"

# 用 package.json 的 name 字段判断是否在模板根（防止在子目录误执行）
if [ ! -f "${TEMPLATE_ROOT}/package.json" ] || ! grep -q '"clipboard-manager"' "${TEMPLATE_ROOT}/package.json" 2>/dev/null; then
    error "必须在模板工程根目录执行此脚本（当前检测不到模板的 package.json）。"
    error "请 cd 到 clipboard-manager 根目录后重试。"
    exit 1
fi

# 检查 rsync（macOS 自带，Linux 一般也有）
if ! command -v rsync >/dev/null 2>&1; then
    error "找不到 rsync，请先安装（macOS 自带，Linux: apt/brew install rsync）。"
    exit 1
fi

echo -e "${CYAN}"
echo "╔══════════════════════════════════════════════╗"
echo "║   Clipboard Manager → 新工程创建向导    ║"
echo "╚══════════════════════════════════════════════╝"
echo -e "${NC}"

# ============================================================
# 交互式收集输入
# ============================================================

# --- 1. 新工程目录路径 ---
read -rp "$(echo -e ${CYAN}'新工程目录路径（如 ~/Documents/infra/my-app）: '${NC})" TARGET_DIR
TARGET_DIR="${TARGET_DIR/#\~/$HOME}"  # 展开 ~

if [ -z "$TARGET_DIR" ]; then
    error "目录路径不能为空。"
    exit 1
fi
if [ -e "$TARGET_DIR" ]; then
    error "目标目录已存在：$TARGET_DIR"
    error "请指定一个不存在的目录，或先删除/改名现有目录。"
    exit 1
fi

# --- 2. 工程名（kebab-case）---
# 默认从目录名推导
DEFAULT_NAME="$(basename "$TARGET_DIR")"
read -rp "$(echo -e ${CYAN}"工程名 (kebab-case) [${DEFAULT_NAME}]: "${NC})" PROJECT_NAME
PROJECT_NAME="${PROJECT_NAME:-$DEFAULT_NAME}"

# 校验 kebab-case：全小写字母/数字/连字符，不以连字符开头/结尾
if ! echo "$PROJECT_NAME" | grep -qE '^[a-z][a-z0-9-]*[a-z0-9]$|^[a-z]$'; then
    error "工程名必须是 kebab-case（全小写字母+数字+连字符，不以连字符开头/结尾）：$PROJECT_NAME"
    exit 1
fi

# --- 3. Bundle identifier ---
DEFAULT_IDENTIFIER="com.example.${PROJECT_NAME}"
read -rp "$(echo -e ${CYAN}"Bundle identifier [${DEFAULT_IDENTIFIER}]: "${NC})" IDENTIFIER
IDENTIFIER="${IDENTIFIER:-$DEFAULT_IDENTIFIER}"

# 校验 identifier：反向域名格式（全小写字母/数字/点/连字符）
if ! echo "$IDENTIFIER" | grep -qE '^[a-z][a-z0-9.-]*[a-z0-9]$'; then
    error "identifier 格式错误（应为反向域名，如 com.company.app，全小写）：$IDENTIFIER"
    exit 1
fi
# 警告 .app 结尾（macOS Finder 会误判）
if [[ "$IDENTIFIER" == *.app ]]; then
    warn "identifier 以 .app 结尾，macOS Finder 会把数据目录误判为应用包。建议改掉。"
    read -rp "$(echo -e ${YELLOW}'仍然继续？[y/N]: '${NC})" CONTINUE
    [[ "$CONTINUE" =~ ^[Yy]$ ]] || exit 1
fi

# --- 4. 产品/显示名 ---
# 默认把 kebab-case 转成 Title Case（ai-image-studio → Ai Image Studio），用户可改
DEFAULT_DISPLAY="$(echo "$PROJECT_NAME" | sed 's/-/ /g' | awk '{for(i=1;i<=NF;i++)$i=toupper(substr($i,1,1))substr($i,2)}1')"
read -rp "$(echo -e ${CYAN}"产品/显示名 [${DEFAULT_DISPLAY}]: "${NC})" DISPLAY_NAME
DISPLAY_NAME="${DISPLAY_NAME:-$DEFAULT_DISPLAY}"

# ============================================================
# 确认
# ============================================================
echo ""
info "将创建新工程："
echo "    目录：       $TARGET_DIR"
echo "    工程名：     $PROJECT_NAME"
echo "    identifier： $IDENTIFIER"
echo "    显示名：     $DISPLAY_NAME"
echo ""
info "占位符替换："
echo "    clipboard-manager  →  $PROJECT_NAME"
echo "    com.example.clipboard-manager    →  $IDENTIFIER"
echo "    Clipboard Manager  →  $DISPLAY_NAME"
echo ""
read -rp "$(echo -e ${YELLOW}'确认创建？[y/N]: '${NC})" CONFIRM
[[ "$CONFIRM" =~ ^[Yy]$ ]] || { info "已取消。"; exit 0; }

# ============================================================
# 1. rsync 复制（排除构建产物/本地配置/git 历史）
# ============================================================
info "复制模板到 $TARGET_DIR …"
mkdir -p "$TARGET_DIR"

rsync -a \
    --exclude '.git' \
    --exclude 'node_modules' \
    --exclude 'src-tauri/target' \
    --exclude 'resources/bin/sidecar' \
    --exclude 'resources/bin/' \
    --exclude 'dist' \
    --exclude '__pycache__' \
    --exclude '.pytest_cache' \
    --exclude '.mypy_cache' \
    --exclude '.ruff_cache' \
    --exclude '.env' \
    --exclude '.env.local' \
    --exclude '.DS_Store' \
    "$TEMPLATE_ROOT"/ "$TARGET_DIR"/

# 确保 resources/bin 目录存在（被打包 binary 的落点，被 exclude 了内容但保留目录）
mkdir -p "$TARGET_DIR/resources/bin"
# .gitkeep 如果被排除规则误伤，补一个
[ -f "$TARGET_DIR/resources/bin/.gitkeep" ] || touch "$TARGET_DIR/resources/bin/.gitkeep"

ok "复制完成"

# ============================================================
# 2. sed 批量替换占位符（3 组）
# ============================================================
info "替换占位符 …"

# 跨平台 sed：macOS BSD sed 和 GNU sed 语法不同（-i 参数）。
# 用 sed -i '' (BSD) 或 sed -i (GNU)。这里用一个兼容写法。
# 定义一个跨平台 in-place 替换函数
sed_inplace() {
    local pattern="$1"
    local replacement="$2"
    local file="$3"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s|${pattern}|${replacement}|g" "$file"
    else
        sed -i "s|${pattern}|${replacement}|g" "$file"
    fi
}

# 需要替换的文件清单（实测扫描确认，见脚本说明书）
# 用 find 收集 + 限定扩展名，避免遗漏新增文件
REPLACED_COUNT=0
replace_in_file() {
    local file="$1"
    # 用 grep 先判断文件是否含占位符（避免无谓写入改动 mtime）
    if grep -qE 'clipboard-manager|com\.example\.template|Clipboard Manager' "$file" 2>/dev/null; then
        sed_inplace 'clipboard-manager' "$PROJECT_NAME" "$file"
        sed_inplace 'com\.example\.template' "$IDENTIFIER" "$file"
        sed_inplace 'Clipboard Manager' "$DISPLAY_NAME" "$file"
        REPLACED_COUNT=$((REPLACED_COUNT + 1))
    fi
}

# 遍历新工程的文本文件（限定常见代码/配置/文档扩展名，跳过二进制/锁文件）
# Cargo.lock / package-lock.json 也替换（里面含工程名）
while IFS= read -r -d '' file; do
    replace_in_file "$file"
done < <(find "$TARGET_DIR" \
    -type f \
    \( \
        -name '*.json' -o \
        -name '*.toml' -o \
        -name '*.ts' -o \
        -name '*.tsx' -o \
        -name '*.py' -o \
        -name '*.rs' -o \
        -name '*.html' -o \
        -name '*.md' -o \
        -name '*.sh' -o \
        -name '*.css' -o \
        -name '*.lock' -o \
        -name '.gitignore' -o \
        -name '*.ini' -o \
        -name '*.mako' \
    \) \
    -not -path '*/node_modules/*' \
    -not -path '*/target/*' \
    -print0)

ok "替换完成，改了 $REPLACED_COUNT 个文件"

# ============================================================
# 3. git init（新工程是独立仓库）
# ============================================================
info "初始化 git …"
cd "$TARGET_DIR"
git init -q
git add -A
git commit -q -m "chore: 基于 clipboard-manager 初始化工程

工程名: $PROJECT_NAME
identifier: $IDENTIFIER
显示名: $DISPLAY_NAME" 2>/dev/null || warn "git commit 失败（可能未配置 user.name/email），请手动 commit"

ok "git 初始化完成"

# ============================================================
# 4. 输出下一步指引
# ============================================================
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║   ✅ 新工程已创建                            ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════╝${NC}"
echo ""
echo "    目录： $TARGET_DIR"
echo ""
info "下一步："
echo "    cd $TARGET_DIR"
echo ""
echo "    # 1. 配置 Python 环境（见 .env.example，设 SIDECAR_DEV_PYTHON）"
echo "    cp .env.example .env"
echo "    # 编辑 .env 填入你的 Python 解释器路径"
echo ""
echo "    # 2. 安装前端依赖"
echo "    npm install"
echo ""
echo "    # 3. 跑起来"
echo "    npm run tauri dev"
echo ""
echo "    # 4. （可选）打包 sidecar binary"
echo "    PYTHON=/path/to/python ./scripts/build-sidecar.sh"
echo ""
warn "提醒：这是模板的子工程，开发成果记得回流模板（见 wiki/process/reflux-signals.md）"
