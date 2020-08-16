// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, fail } from "./unit/test_util.ts";

Deno.test("invalid scheme", () => {
  assertThrows(() => new WebSocket("foo://localhost:4242"));
});

Deno.test("fragment", () => {
  assertThrows(() => new WebSocket("ws://localhost:4242/#"));
  assertThrows(() => new WebSocket("ws://localhost:4242/#foo"));
});

Deno.test("duplicate protocols", () => {
  assertThrows(() => new WebSocket("ws://localhost:4242", ["foo", "foo"]));
});

Deno.test("invalid server", () => {
  const ws = new WebSocket("ws://localhost:2121");
  let i = 0;
  ws.onerror = (): void => {
    i++;
  };
  ws.onclose = (): void => {
    if (i !== 1) fail();
  };
  ws.onopen = (): void => fail();
});

Deno.test("connect & close", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.close();
});

Deno.test("connect & close custom valid code", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.close(1000);
});

Deno.test("connect & close custom invalid code", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => {
    assertThrows(() => ws.close(1001));
  };
});

Deno.test("connect & close custom valid reason", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.close(1000, "foo");
});

Deno.test("connect & close custom invalid reason", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => {
    assertThrows(() => ws.close(1000, "".padEnd(124, "o")));
  };
});

Deno.test("echo string", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send("foo");
  ws.onmessage = (e): void => assertEquals(e.data, "foo");
});

Deno.test("echo blob with binaryType blob", () => {
  const ws = new WebSocket("ws://localhost:4242");
  const blob = new Blob(["foo"]);
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send(blob);
  ws.onmessage = async (e): Promise<void> =>
    assertEquals(await e.data.text(), await blob.text());
});

Deno.test("echo blob with binaryType arraybuffer", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const blob = new Blob(["foo"]);
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send(blob);
  ws.onmessage = async (e): Promise<void> =>
    assertEquals(await e.data.arrayBuffer(), await blob.arrayBuffer());
});

Deno.test("echo uint8array with binaryType blob", () => {
  const ws = new WebSocket("ws://localhost:4242");
  const uint = new Uint8Array([102, 111, 111]);
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send(uint);
  ws.onmessage = async (e): Promise<void> =>
    assertEquals(await e.data.arrayBuffer(), uint.buffer);
});

Deno.test("echo uint8array with binaryType arraybuffer", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const uint = new Uint8Array([102, 111, 111]);
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send(uint);
  ws.onmessage = (e): void => assertEquals(e.data, uint.buffer);
});

Deno.test("echo arraybuffer with binaryType blob", () => {
  const ws = new WebSocket("ws://localhost:4242");
  const buffer = new ArrayBuffer(3);
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send(buffer);
  ws.onmessage = async (e): Promise<void> =>
    assertEquals(await e.data.arrayBuffer(), buffer);
});

Deno.test("echo arraybuffer with binaryType arraybuffer", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const buffer = new ArrayBuffer(3);
  ws.onerror = (): void => fail();
  ws.onopen = (): void => ws.send(buffer);
  ws.onmessage = (e): void => assertEquals(e.data, buffer);
});

Deno.test("send setinterval", () => {
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = (): void => fail();
  ws.onopen = (): void => {
    let i = 0;
    const interval = setInterval(() => {
      ws.send("foo");
      if (i == 10) {
        clearInterval(interval);
      }
      i++;
    }, 100);
  };
});
