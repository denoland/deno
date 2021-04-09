// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrowsAsync,
  unitTest,
} from "./test_util.ts";
import { BufReader, BufWriter } from "../../../test_util/std/io/bufio.ts";
import { TextProtoReader } from "../../../test_util/std/textproto/mod.ts";

unitTest({ perms: { net: true } }, async function httpServerBasic() {
  const promise = (async () => {
    const listener = Deno.listen({ port: 4501 });
    for await (const conn of listener) {
      const httpConn = Deno.serveHttp(conn);
      for await (const { request, respondWith } of httpConn) {
        assertEquals(await request.text(), "");
        respondWith(new Response("Hello World"));
      }
      break;
    }
  })();

  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  const text = await resp.text();
  assertEquals(text, "Hello World");
  await promise;
});

unitTest(
  { perms: { net: true } },
  async function httpServerStreamResponse() {
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();

    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { request, respondWith } = evt;
      assert(!request.body);
      await respondWith(new Response(stream.readable));
      httpConn.close();
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();
    assertEquals("hello world", respBody);
    await promise;
  },
);

unitTest(
  { perms: { net: true } },
  async function httpServerStreamRequest() {
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();

    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { request, respondWith } = evt;
      const reqBody = await request.text();
      assertEquals("hello world", reqBody);
      await respondWith(new Response(""));

      // TODO(ry) If we don't call httpConn.nextRequest() here we get "error sending
      // request for url (https://localhost:4501/): connection closed before
      // message completed".
      assertEquals(await httpConn.nextRequest(), null);

      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/", {
      body: stream.readable,
      method: "POST",
      headers: { "connection": "close" },
    });

    await resp.arrayBuffer();
    await promise;
  },
);

unitTest({ perms: { net: true } }, async function httpServerStreamDuplex() {
  const promise = (async () => {
    const listener = Deno.listen({ port: 4501 });
    const conn = await listener.accept();
    const httpConn = Deno.serveHttp(conn);
    const evt = await httpConn.nextRequest();
    assert(evt);
    const { request, respondWith } = evt;
    assert(request.body);
    await respondWith(new Response(request.body));
    httpConn.close();
    listener.close();
  })();

  const ts = new TransformStream();
  const writable = ts.writable.getWriter();
  const resp = await fetch("http://127.0.0.1:4501/", {
    method: "POST",
    body: ts.readable,
  });
  assert(resp.body);
  const reader = resp.body.getReader();
  await writable.write(new Uint8Array([1]));
  const chunk1 = await reader.read();
  assert(!chunk1.done);
  assertEquals(chunk1.value, new Uint8Array([1]));
  await writable.write(new Uint8Array([2]));
  const chunk2 = await reader.read();
  assert(!chunk2.done);
  assertEquals(chunk2.value, new Uint8Array([2]));
  await writable.close();
  const chunk3 = await reader.read();
  assert(chunk3.done);
  await promise;
});

unitTest({ perms: { net: true } }, async function httpServerClose() {
  const listener = Deno.listen({ port: 4501 });
  const client = await Deno.connect({ port: 4501 });
  const httpConn = Deno.serveHttp(await listener.accept());
  client.close();
  const evt = await httpConn.nextRequest();
  assertEquals(evt, null);
  // Note httpConn is automatically closed when "done" is reached.
  listener.close();
});

unitTest({ perms: { net: true } }, async function httpServerInvalidMethod() {
  const listener = Deno.listen({ port: 4501 });
  const client = await Deno.connect({ port: 4501 });
  const httpConn = Deno.serveHttp(await listener.accept());
  await client.write(new Uint8Array([1, 2, 3]));
  await assertThrowsAsync(
    async () => {
      await httpConn.nextRequest();
    },
    Deno.errors.Http,
    "invalid HTTP method parsed",
  );
  // Note httpConn is automatically closed when it errors.
  client.close();
  listener.close();
});

unitTest(
  { perms: { read: true, net: true } },
  async function httpServerWithTls(): Promise<void> {
    const hostname = "localhost";
    const port = 4501;

    const promise = (async () => {
      const listener = Deno.listenTls({
        hostname,
        port,
        certFile: "cli/tests/tls/localhost.crt",
        keyFile: "cli/tests/tls/localhost.key",
      });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { request, respondWith } = evt;
      await respondWith(new Response("Hello World"));

      // TODO(ry) If we don't call httpConn.nextRequest() here we get "error sending
      // request for url (https://localhost:4501/): connection closed before
      // message completed".
      assertEquals(await httpConn.nextRequest(), null);

      listener.close();
    })();

    const caData = Deno.readTextFileSync("cli/tests/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caData });
    const resp = await fetch(`https://${hostname}:${port}/`, {
      client,
      headers: { "connection": "close" },
    });
    const respBody = await resp.text();
    assertEquals("Hello World", respBody);
    await promise;
    client.close();
  },
);
