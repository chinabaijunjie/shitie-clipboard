"""
事件总线 — SSE 进度推送用

- 后端业务代码 emit 事件（任务进度等）
- /api/events/{channel} 端点 subscribe 队列，通过 SSE 推给前端

业务工程起步时，在 Channels 追加业务频道常量（如 IMAGE_GEN_PROGRESS）。
"""

from __future__ import annotations

import asyncio
import logging
from collections import defaultdict, deque
from collections.abc import AsyncGenerator, Callable
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any

_LOGGER = logging.getLogger(__name__)


@dataclass(frozen=True)
class Event:
    """统一事件载荷。"""

    type: str
    data: dict[str, Any]
    timestamp: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


class EventBus:
    """
    简单 in-memory pub/sub。

    每个 subscriber 拥有独立 asyncio.Queue，背压上限 100 条，
    超过后丢弃最旧事件并打 log（避免慢消费者拖垮整个系统）。
    """

    BACKLOG_PER_SUBSCRIBER: int = 100

    def __init__(self) -> None:
        self._subscribers: dict[str, deque[asyncio.Queue[Event]]] = defaultdict(deque)
        self._lock = asyncio.Lock()

    async def subscribe(self, channel: str) -> asyncio.Queue[Event]:
        queue: asyncio.Queue[Event] = asyncio.Queue(maxsize=self.BACKLOG_PER_SUBSCRIBER)
        async with self._lock:
            self._subscribers[channel].append(queue)
        return queue

    async def unsubscribe(self, channel: str, queue: asyncio.Queue[Event]) -> None:
        async with self._lock:
            if queue in self._subscribers[channel]:
                self._subscribers[channel].remove(queue)

    async def publish(self, channel: str, event: Event) -> None:
        async with self._lock:
            queues = list(self._subscribers[channel])
        for q in queues:
            try:
                q.put_nowait(event)
            except asyncio.QueueFull:
                # 慢消费者，丢弃最旧的事件以腾出空间
                try:
                    q.get_nowait()
                    q.put_nowait(event)
                except asyncio.QueueFull:
                    _LOGGER.warning("EventBus 事件投递失败（channel=%s）", channel)
                except Exception:
                    _LOGGER.exception("EventBus 事件投递异常（channel=%s）", channel)

    async def stream(
        self,
        channel: str,
    ) -> AsyncGenerator[Event, None]:
        """生成器：从频道持续读取事件，直到消费者取消。"""
        queue = await self.subscribe(channel)
        try:
            while True:
                event = await queue.get()
                yield event
        finally:
            await self.unsubscribe(channel, queue)

    def subscriber_count(self, channel: str | None = None) -> int:
        if channel is None:
            return sum(len(qs) for qs in self._subscribers.values())
        return len(self._subscribers[channel])


# ============================================================
# 全局单例
# ============================================================

event_bus = EventBus()


# ============================================================
# 便捷 emit 函数
# ============================================================

# 维护对 pending task 的强引用，防止 Python GC 在 task 完成前回收
# （asyncio 文档明确警告 loop.create_task 返回的 task 若无引用可能被 GC）。
# task 完成后通过 done_callback 自动从 set 中移除，无内存泄漏。
_pending_tasks: set[asyncio.Task[None]] = set()


def emit(channel: str, event_type: str, data: dict[str, Any] | None = None) -> None:
    """同步 emit，内部 schedule 到事件循环。"""
    payload = Event(type=event_type, data=data or {})
    try:
        loop = asyncio.get_running_loop()
        task = loop.create_task(event_bus.publish(channel, payload))
        _pending_tasks.add(task)
        task.add_done_callback(_pending_tasks.discard)
    except RuntimeError as e:
        # 没有 event loop 的场景（早期初始化 / 同步上下文）— log 而非静默吞
        if "no running event loop" in str(e):
            _LOGGER.debug(
                "emit 在无 running loop 中调用，事件已丢弃（channel=%s, type=%s）",
                channel,
                event_type,
            )
        else:
            raise


# 事件频道常量（业务工程起步时在此追加业务频道）
class Channels:
    pass


# 类型 alias
EventListener = Callable[[Event], None]
