// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals } from "./test_util.ts";

async function assertCompactChunks(
  stream: ReadableStream<Uint8Array>,
  expectedSize: number,
) {
  const chunks: Uint8Array[] = [];
  for await (const chunk of stream) {
    chunks.push(chunk);
  }
  assertEquals(
    chunks.reduce((size, chunk) => size + chunk.length, 0),
    expectedSize,
  );
  for (const chunk of chunks) {
    assert(chunk.every((byte) => byte === 97));
    assertEquals(chunk.buffer.byteLength, chunk.byteLength);
  }
}

for (const size of [5, 100, 65536, 65537]) {
  Deno.test({
    name: `resourceStreamCompactFileChunks_${size}`,
    permissions: { read: true, write: true },
    async fn() {
      const path = await Deno.makeTempFile();
      try {
        await Deno.writeFile(path, new Uint8Array(size).fill(97));
        const file = await Deno.open(path);
        await assertCompactChunks(file.readable, size);
      } finally {
        await Deno.remove(path);
      }
    },
  });
}

Deno.test({
  name: "resourceStreamCompactFetchChunks",
  permissions: { net: true },
  async fn() {
    const server = Deno.serve({ port: 0, onListen() {} }, () => {
      let remaining = 128;
      return new Response(
        new ReadableStream({
          pull(controller) {
            if (remaining-- > 0) {
              controller.enqueue(new Uint8Array(1000).fill(97));
            } else {
              controller.close();
            }
          },
        }),
      );
    });
    try {
      const response = await fetch(`http://localhost:${server.addr.port}/`);
      assert(response.body);
      await assertCompactChunks(response.body, 128000);
    } finally {
      await server.shutdown();
    }
  },
});

Deno.test({
  name: "resourceStreamCompactSocketChunks",
  permissions: { net: true },
  async fn() {
    using listener = Deno.listen({ port: 0 });
    const connecting = Deno.connect({ port: listener.addr.port });
    using server = await listener.accept();
    const client = await connecting;
    await server.write(new Uint8Array(100).fill(97));
    await server.closeWrite();
    await assertCompactChunks(client.readable, 100);
  },
});

Deno.test({
  name: "resourceStreamPreservesBYOBBuffer",
  permissions: { read: true, write: true },
  async fn() {
    const path = await Deno.makeTempFile();
    try {
      await Deno.writeTextFile(path, "hello");
      const file = await Deno.open(path);
      const reader = file.readable.getReader({ mode: "byob" });
      try {
        const buffer = new ArrayBuffer(64);
        const { done, value } = await reader.read(
          new Uint8Array(buffer, 8, 32),
        );
        assert(!done);
        assertEquals(new TextDecoder().decode(value), "hello");
        assertEquals(value.byteOffset, 8);
        assertEquals(value.buffer.byteLength, 64);
        assertEquals(buffer.byteLength, 0);
        const end = await reader.read(new Uint8Array(value.buffer, 8, 32));
        assert(end.done);
        assertEquals(end.value?.buffer.byteLength, 64);
      } finally {
        await reader.cancel();
      }
    } finally {
      await Deno.remove(path);
    }
  },
});

Deno.test({
  name: "resourceStreamPreservesSocketBYOBBuffer",
  permissions: { net: true },
  async fn() {
    using listener = Deno.listen({ port: 0 });
    const connecting = Deno.connect({ port: listener.addr.port });
    using server = await listener.accept();
    const client = await connecting;
    await server.write(new Uint8Array(100).fill(97));
    await server.closeWrite();
    const reader = client.readable.getReader({ mode: "byob" });
    try {
      let buffer = new ArrayBuffer(64);
      let received = 0;
      while (true) {
        const { value, done } = await reader.read(
          new Uint8Array(buffer, 8, 32),
        );
        assert(value);
        assertEquals(buffer.byteLength, 0);
        assertEquals(value.byteOffset, 8);
        assertEquals(value.buffer.byteLength, 64);
        buffer = value.buffer;
        if (done) break;
        assert(value.every((byte) => byte === 97));
        received += value.length;
      }
      assertEquals(received, 100);
    } finally {
      await reader.cancel();
    }
  },
});
