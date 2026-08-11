# scripts/ 说明书

> 本目录是拾贴的工具脚本目录。**给人和大模型看的说明书**——看不懂脚本代码时,读这里。

---

## hooks/pre-commit — 双工具同步检查

检查 `CLAUDE.md` ↔ `AGENTS.md` 同步状态(AGENTS.md 改动除 §8 外必须同步到 CLAUDE.md)。

### 设计

1. 只在 `CLAUDE.md` 或 `AGENTS.md` 在暂存区时触发(其他改动放行)
2. 对两个文件做 normalization(消除已知允许的差异:AGENTS.md §8 受保护段、视角词、时间戳等)
3. normalization 后做 diff,应该为空
4. 不为空 → 列出未同步差异 + 阻塞 commit
5. 兜底:确认为误报时 `git commit --no-verify` 跳过,并在 `wiki/process/dual-tool-sync.md` 补规则

### 安装

```bash
cd <工程根目录>
ln -sf ../../scripts/hooks/pre-commit .git/hooks/pre-commit
```

详见 `wiki/process/dual-tool-sync.md`。

---

## 历史脚本(已删)

M0 架构清理时删除了以下模板母体专用脚本(本工程作为子工程不再需要):

- `init-project.sh` — 模板母体创建新子工程用,子工程本身不需要
- `build-sidecar.sh` — PyInstaller 打包 sidecar binary,sidecar 架构已删除

> 这些脚本在模板母体仍保留。
