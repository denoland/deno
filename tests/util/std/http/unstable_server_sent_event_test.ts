// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../assert/mod.ts";
import {
  ServerSentEvent,
  ServerSentEventStreamTarget,
} from "./unstable_server_sent_event.ts";

Deno.test({
  name: "ServerSentEvent - construction",
  fn() {
    const evt = new ServerSentEvent("message", { data: "foobar" });
    assertEquals(evt.type, "message");
    assertEquals(evt.data, "foobar");
    assertEquals(evt.id, undefined);
    assertEquals(String(evt), `event: message\ndata: foobar\n\n`);
  },
});

Deno.test({
  name: "ServerSentEvent - data coercion",
  fn() {
    const evt = new ServerSentEvent("ping", { data: { hello: true } });
    assertEquals(evt.type, "ping");
    assertEquals(evt.data, `{"hello":true}`);
    assertEquals(evt.id, undefined);
    assertEquals(String(evt), `event: ping\ndata: {"hello":true}\n\n`);
  },
});

Deno.test({
  name: "ServerSentEvent - init id",
  fn() {
    const evt = new ServerSentEvent("ping", { data: "foobar", id: 1234 });
    assertEquals(evt.type, "ping");
    assertEquals(evt.data, `foobar`);
    assertEquals(evt.id, 1234);
    assertEquals(
      String(evt),
      `event: ping\nid: 1234\ndata: foobar\n\n`,
    );
  },
});

Deno.test({
  name: "ServerSentEvent - data space",
  fn() {
    const evt = new ServerSentEvent("ping", {
      data: { hello: [1, 2, 3] },
      space: 2,
    });
    assertEquals(evt.type, "ping");
    assertEquals(evt.data, `{\n  "hello": [\n    1,\n    2,\n    3\n  ]\n}`);
    assertEquals(
      String(evt),
      `event: ping\ndata: {\ndata:   "hello": [\ndata:     1,\ndata:     2,\ndata:     3\ndata:   ]\ndata: }\n\n`,
    );
  },
});

Deno.test({
  name: "ServerSentEvent - __message",
  fn() {
    const evt = new ServerSentEvent("__message", { data: { hello: "world" } });
    assertEquals(evt.type, "__message");
    assertEquals(evt.data, `{"hello":"world"}`);
    assertEquals(String(evt), `data: {"hello":"world"}\n\n`);
  },
});

Deno.test({
  name: "ServerSentEvent - without eventInit",
  fn() {
    const evt = new ServerSentEvent("reload");
    assertEquals(evt.type, "reload");
    assertEquals(evt.data, "");
    assertEquals(String(evt), `event: reload\ndata: \n\n`);
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - construction",
  async fn() {
    const sse = new ServerSentEventStreamTarget();
    assertEquals(sse.closed, false);
    const response = sse.asResponse();
    await sse.close();
    assert(response.body);
    const reader = response.body.getReader();
    await reader.closed;
    assertEquals(response.status, 200);
    assertEquals(response.headers.get("content-type"), "text/event-stream");
    assertEquals(response.headers.get("connection"), "Keep-Alive");
    assertEquals(
      response.headers.get("keep-alive"),
      "timeout=9007199254740991",
    );
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - construction with headers",
  async fn() {
    const sse = new ServerSentEventStreamTarget();
    const response = sse.asResponse({
      headers: new Headers([["X-Deno", "test"], ["Cache-Control", "special"]]),
    });
    await sse.close();
    assertEquals(response.headers.get("content-type"), "text/event-stream");
    assertEquals(response.headers.get("connection"), "Keep-Alive");
    assertEquals(response.headers.get("x-deno"), "test");
    assertEquals(response.headers.get("cache-control"), "no-cache");
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - dispatchEvent",
  async fn() {
    const sse = new ServerSentEventStreamTarget();
    const response = sse.asResponse();
    const evt = new ServerSentEvent("message", { data: "foobar" });
    sse.dispatchEvent(evt);
    await sse.close();
    assertEquals(await response.text(), "event: message\ndata: foobar\n\n");
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - dispatchMessage",
  async fn() {
    const sse = new ServerSentEventStreamTarget();
    const response = sse.asResponse();
    sse.dispatchMessage("foobar");
    await sse.close();
    assertEquals(await response.text(), "data: foobar\n\n");
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - dispatchComment",
  async fn() {
    const sse = new ServerSentEventStreamTarget();
    const response = sse.asResponse();
    sse.dispatchComment("foobar");
    await sse.close();
    assertEquals(await response.text(), ": foobar\n\n");
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - keep-alive setting",
  fn() {
    const sse = new ServerSentEventStreamTarget({ keepAlive: 1000 });
    const response = sse.asResponse();
    const p = new Promise<void>((resolve, reject) => {
      setTimeout(async () => {
        try {
          await sse.close();
          assertEquals(await response.text(), ": keep-alive comment\n\n");
          resolve();
        } catch (e) {
          reject(e);
        }
      }, 1250);
    });
    return p;
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - connection closed readable stream",
  fn() {
    let closed = false;
    let errored = false;
    const sse = new ServerSentEventStreamTarget();
    const response = sse.asResponse();
    sse.addEventListener("close", () => {
      closed = true;
    });
    sse.addEventListener("error", () => {
      errored = true;
    });
    assert(response.body);
    response.body.cancel(
      new Error("connection closed before message completed"),
    );
    assert(closed);
    assert(!errored);
  },
});

Deno.test({
  name: "ServerSentEventStreamTarget - inspecting",
  fn() {
    assertEquals(
      Deno.inspect(new ServerSentEventStreamTarget()),
      `ServerSentEventStreamTarget { "#bodyInit": ReadableStream { locked: false }, "#closed": false }`,
    );
  },
});
