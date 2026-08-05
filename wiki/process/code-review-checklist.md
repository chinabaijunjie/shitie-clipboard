---
title: Code Review 检查清单
tags: [review, 清单, 质量]
created: 2026-06-29
updated: 2026-06-29
status: active
---

# Code Review 检查清单

> **触发场景**：commit 前 / 每个 Stage 完成后自检。
>
> **两道防线**：本清单（第一道，自检）+ 深度 review（第二道，feature 分支合并 master 前用
> 多 agent 并行深度 review）。

## Python 后端检查清单

- [ ] **类型注解**：所有函数有参数 + 返回值类型注解
- [ ] **错误处理**：异常显式处理，**禁止静默吞错**（无空 `except:` / 无 `except: pass`）
- [ ] **异步规范**：async 函数内无阻塞调用（阻塞 IO 用 `asyncio.to_thread`）
- [ ] **SQLAlchemy**：用 async session，无同步引擎混用；查询用 `select()` 2.0 风格
- [ ] **日志**：用 `core.logger.get_logger()`，不用 `print()`；记录完整上下文
- [ ] **配置外置**：无硬编码值（端口/路径/阈值用 config.py 或环境变量）
- [ ] **领域资源校验**：系统边界（用户输入/API 响应/文件内容/模型输出）进入即校验
  （业务工程填充领域专属校验，如文件完整性 / 数据格式 / 资源可用性）

## TypeScript 前端检查清单

- [ ] **类型安全**：无 `any`（用 `unknown` + 类型守卫），props 有 interface
- [ ] **组件封装**：用项目 UI 组件层，业务逻辑不写在视图组件里
- [ ] **状态管理**：全局状态用状态管理库（项目用 Zustand），组件内状态用 useState
- [ ] **API 调用**：走统一 API client 层（`@core/api`），不直接 fetch/axios
- [ ] **错误边界**：API 调用有 try/catch + 友好提示（useNotify），不静默吞错

## 通用检查清单

- [ ] **可读 / 命名清晰**
- [ ] **函数 < 50 行**
- [ ] **文件 < 800 行**（超出考虑拆分）
- [ ] **嵌套不超过 4 层**
- [ ] **不可变性**：没有原地修改对象/state（React state 用展开运算符）
- [ ] **无硬编码密钥**：API key / token / 密码走环境变量或 config
- [ ] **无冗余**：无重复代码 / 无死代码
- [ ] **接口一致**：前后端契约对齐(invoke 错误码镜像、command 参数、数据模型两端 serde 对齐)
- [ ] **注释描述 WHY**：复杂逻辑有注释说明为什么这么写（不是写什么）
- [ ] **测试覆盖错误路径**：不只测 happy path，异常分支也要测

## Review 执行方式

1. 逐项过清单
2. 发现问题当场修复
3. commit message 附 review 结果（如"review: 全部通过"或"review: 修了 X 项"）
