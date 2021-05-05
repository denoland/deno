import { assertEquals } from "./test_util/std/testing/asserts.ts";

const ws = new WebSocketStream("ws://echo.websocket.org");
const { readable, writable } = await ws.connection;
await writable.getWriter().write("foo");
const res = await readable.getReader().read();
assertEquals(res.value, "foo");
ws.close();
await ws.closed;
