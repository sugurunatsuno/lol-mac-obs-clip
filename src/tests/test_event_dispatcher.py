# tests/test_event_dispatcher.py

import pytest
import asyncio
from utils.event_dispatcher import EventDispatcher
from utils.event_types import EventType

@pytest.mark.asyncio
async def test_register_adds_handler():
    dispatcher = EventDispatcher()

    async def handler(event): pass

    dispatcher.register(EventType.CHAMPION_KILL, handler)
    await dispatcher.dispatch(EventType.CHAMPION_KILL, {"test": True})
    # 明示的なassertはないけど、例外が出なければOK！

@pytest.mark.asyncio
async def test_dispatch_calls_handler():
    dispatcher = EventDispatcher()
    called = False

    async def handler(event):
        nonlocal called
        called = True

    dispatcher.register("CustomEvent", handler)
    await dispatcher.dispatch("CustomEvent", {"data": 123})
    await asyncio.sleep(0.1)
    assert called is True

@pytest.mark.asyncio
async def test_dispatch_multiple_handlers():
    dispatcher = EventDispatcher()
    calls = []

    async def handler1(event):
        calls.append("h1")

    async def handler2(event):
        calls.append("h2")

    dispatcher.register("MultiEvent", handler1)
    dispatcher.register("MultiEvent", handler2)

    await dispatcher.dispatch("MultiEvent", {})
    await asyncio.sleep(0.1)
    assert "h1" in calls and "h2" in calls

@pytest.mark.asyncio
async def test_dispatch_with_no_handlers():
    dispatcher = EventDispatcher()
    await dispatcher.dispatch("NoHandlerEvent", {"dummy": True})
    # エラー出なければOK！

@pytest.mark.asyncio
async def test_dispatch_handles_exception(caplog):
    dispatcher = EventDispatcher()

    async def faulty_handler(event):
        raise RuntimeError("Oops!")

    dispatcher.register("ErrorEvent", faulty_handler)
    await dispatcher.dispatch("ErrorEvent", {})
    await asyncio.sleep(0.1)

    assert any("RuntimeError" in r.message for r in caplog.records)