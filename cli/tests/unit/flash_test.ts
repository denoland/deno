// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

import {
  Buffer,
  BufReader,
  BufWriter,
} from "../../../test_util/std/io/buffer.ts";
import { TextProtoReader } from "../../../test_util/std/textproto/mod.ts";
import { serve, serveTls } from "../../../test_util/std/http/server.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertStrictEquals,
  assertThrows,
  deferred,
  delay,
  fail,
} from "./test_util.ts";

Deno.test({ permissions: { net: true } }, async function httpServerBasic() {
  const ac = new AbortController();

  const promise = (async () => {
    await Deno.serve(async (request) => {
      assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
      assertEquals(await request.text(), "");
      return new Response("Hello World", { headers: { "foo": "bar" } });
    }, { port: 4501, signal: ac.signal });
  })();

  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await promise;
});

// https://github.com/denoland/deno/issues/15107
Deno.test(
  { permissions: { net: true } },
  async function httpLazyHeadersIssue15107() {
    const promise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve(async (request) => {
      await request.text();
      headers = request.headers;
      promise.resolve();
      return new Response("");
    }, { port: 2333, signal: ac.signal });

    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();
    assertEquals(headers!.get("content-length"), "5");
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpReadHeadersAfterClose() {
    const promise = deferred();
    const ac = new AbortController();

    let req: Request;
    const server = Deno.serve(async (request) => {
      await request.text();
      req = request;
      promise.resolve();
      return new Response("Hello World");
    }, { port: 2334, signal: ac.signal });

    const conn = await Deno.connect({ port: 2334 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();

    assertThrows(
      () => {
        req.headers;
      },
      TypeError,
      "request closed",
    );

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerGetRequestBody() {
    const promise = deferred();
    const ac = new AbortController();

    const server = Deno.serve((request) => {
      assertEquals(request.body, null);
      promise.resolve();
      return new Response("", { headers: {} });
    }, { port: 4501, signal: ac.signal });

    const conn = await Deno.connect({ port: 4501 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const resp = new Uint8Array(200);
    const readResult = await conn.read(resp);
    assert(readResult > 0);

    conn.close();
    await promise;
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamResponse() {
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();

    // const promise = deferred();
    const ac = new AbortController();

    const server = Deno.serve((request) => {
      // assert(!request.body);
      return new Response(stream.readable);
    }, { port: 4501, signal: ac.signal });

    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();
    assertEquals("hello world", respBody);
    ac.abort();
    await server;
  },
);
