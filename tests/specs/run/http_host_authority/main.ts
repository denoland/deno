// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-deprecated-deno-api

import { createServer } from "node:http";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function assertEquals(actual: unknown, expected: unknown) {
  if (actual !== expected) {
    throw new Error(`Expected ${expected}, got ${actual}`);
  }
}

function concat(parts: Uint8Array[]): Uint8Array {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const result = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}

function request(
  host: Uint8Array | null,
  extraHeaders: Uint8Array = new Uint8Array(),
): Uint8Array {
  return concat([
    encoder.encode("GET / HTTP/1.1\r\n"),
    ...(host === null
      ? []
      : [encoder.encode("Host: "), host, encoder.encode("\r\n")]),
    extraHeaders,
    encoder.encode("Connection: close\r\n\r\n"),
  ]);
}

async function writeAll(conn: Deno.Conn, data: Uint8Array) {
  let offset = 0;
  while (offset < data.length) {
    offset += await conn.write(data.subarray(offset));
  }
}

async function send(port: number, data: Uint8Array): Promise<string> {
  const conn = await Deno.connect({ hostname: "127.0.0.1", port });
  try {
    await writeAll(conn, data);
    const chunks: Uint8Array[] = [];
    const buffer = new Uint8Array(1024);
    while (true) {
      const read = await conn.read(buffer);
      if (read === null) break;
      chunks.push(buffer.slice(0, read));
    }
    return decoder.decode(concat(chunks));
  } finally {
    conn.close();
  }
}

function assertStatus(response: string, status: number) {
  assertEquals(response.split("\r\n", 1)[0], `HTTP/1.1 ${status} ${
    status === 200 ? "OK" : "Bad Request"
  }`);
}

async function testDenoServe() {
  const listening = Promise.withResolvers<number>();
  const ac = new AbortController();
  let calls = 0;
  let observedHeader: string | null = null;
  await using server = Deno.serve({
    hostname: "127.0.0.1",
    port: 0,
    signal: ac.signal,
    onListen: ({ port }) => listening.resolve(port),
  }, (req) => {
    calls++;
    observedHeader = req.headers.get("x-byte-value") ?? observedHeader;
    return new Response();
  });
  const port = await listening.promise;

  for (const host of [
    "example.com",
    "xn--tda.com",
    "127.0.0.1",
    "127.0.0.1:8080",
    "[::1]",
    "[2001:db8::1]:8080",
  ]) {
    assertStatus(await send(port, request(encoder.encode(host))), 200);
  }
  assertStatus(await send(port, request(new Uint8Array())), 200);
  assertStatus(await send(port, request(null)), 200);
  assertStatus(
    await send(
      port,
      request(
        encoder.encode("example.com"),
        concat([
          encoder.encode("X-Byte-Value: "),
          new Uint8Array([0xc3, 0xbc]),
          encoder.encode("\r\n"),
        ]),
      ),
    ),
    200,
  );
  assertEquals(observedHeader, "Ã¼");

  for (const host of [
    new Uint8Array([0xc3, 0xbc, 0x2e, 0x63, 0x6f, 0x6d]),
    new Uint8Array([0xff, 0x2e, 0x63, 0x6f, 0x6d]),
    encoder.encode("user@example.com"),
    encoder.encode("example.com:http"),
    encoder.encode("[not-an-ip]"),
  ]) {
    assertStatus(await send(port, request(host)), 400);
  }
  assertStatus(
    await send(
      port,
      concat([
        encoder.encode("GET / HTTP/1.1\r\n"),
        encoder.encode("Host: example.com\r\n"),
        encoder.encode("Host: example.org\r\n"),
        encoder.encode("Connection: close\r\n\r\n"),
      ]),
    ),
    400,
  );
  assertStatus(
    await send(
      port,
      encoder.encode(
        "GET / HTTP/1.0\r\nHost: example.com\r\nHost: example.org\r\nConnection: close\r\n\r\n",
      ),
    ),
    400,
  );
  assertStatus(
    await send(
      port,
      request(
        new Uint8Array([0xc3, 0xbc, 0x2e, 0x63, 0x6f, 0x6d]),
        encoder.encode("Expect: 100-continue\r\nContent-Length: 1\r\n"),
      ),
    ),
    400,
  );
  assertEquals(calls, 9);

  ac.abort();
  await server.finished;
  console.log("Deno.serve: ok");
}

async function testDenoServeHttp() {
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
  const port = (listener.addr as Deno.NetAddr).port;
  let calls = 0;

  async function serveOne(data: Uint8Array): Promise<string> {
    const accepted = (async () => {
      const conn = await listener.accept();
      // @ts-ignore `Deno.serveHttp()` was soft-removed in Deno 2.
      const httpConn = Deno.serveHttp(conn);
      try {
        const event = await httpConn.nextRequest();
        if (event !== null) {
          calls++;
          await event.respondWith(new Response());
          await httpConn.nextRequest();
        }
      } finally {
        httpConn.close();
      }
    })();
    const response = await send(port, data);
    await accepted;
    return response;
  }

  assertStatus(
    await serveOne(request(new Uint8Array([0xc3, 0xbc, 0x2e, 0x63, 0x6f, 0x6d]))),
    400,
  );
  assertStatus(
    await serveOne(
      request(new Uint8Array([0xff, 0x2e, 0x63, 0x6f, 0x6d])),
    ),
    400,
  );
  assertStatus(
    await serveOne(
      request(
        encoder.encode("example.com"),
        encoder.encode("Host: example.org\r\n"),
      ),
    ),
    400,
  );
  assertStatus(
    await serveOne(request(encoder.encode("example.com"))),
    200,
  );
  assertEquals(calls, 1);
  listener.close();
  console.log("Deno.serveHttp: ok");
}

async function testNodeHttp() {
  let calls = 0;
  let observedHeader: string | string[] | undefined;
  const server = createServer((req, res) => {
    calls++;
    observedHeader = req.headers["x-byte-value"];
    res.end();
  });
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("Expected a TCP address");
  }

  assertStatus(
    await send(
      address.port,
      request(new Uint8Array([0xc3, 0xbc, 0x2e, 0x63, 0x6f, 0x6d])),
    ),
    400,
  );
  assertStatus(
    await send(
      address.port,
      request(new Uint8Array([0xff, 0x2e, 0x63, 0x6f, 0x6d])),
    ),
    400,
  );
  assertStatus(
    await send(
      address.port,
      request(
        encoder.encode("example.com"),
        encoder.encode("Host: example.org\r\n"),
      ),
    ),
    400,
  );
  assertStatus(
    await send(
      address.port,
      request(
        encoder.encode("example.com"),
        concat([
          encoder.encode("X-Byte-Value: "),
          new Uint8Array([0xc3, 0xbc]),
          encoder.encode("\r\n"),
        ]),
      ),
    ),
    200,
  );
  assertEquals(observedHeader, "Ã¼");
  assertEquals(calls, 1);
  await new Promise<void>((resolve, reject) => {
    server.close((error) => error ? reject(error) : resolve());
  });
  console.log("node:http: ok");
}

await testDenoServe();
await testDenoServeHttp();
await testNodeHttp();
