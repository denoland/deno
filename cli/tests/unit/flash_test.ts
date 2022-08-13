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

    const ac = new AbortController();

    const server = Deno.serve((request) => {
      assert(!request.body);
      return new Response(stream.readable);
    }, { port: 4501, signal: ac.signal });

    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();
    assertEquals("hello world", respBody);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamRequest() {
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();

    const ac = new AbortController();
    const server = Deno.serve(async (request) => {
      const reqBody = await request.text();
      assertEquals("hello world", reqBody);
      return new Response("yo");
    }, { port: 4501, signal: ac.signal });

    const resp = await fetch("http://127.0.0.1:4501/", {
      body: stream.readable,
      method: "POST",
      headers: { "connection": "close" },
    });

    assertEquals(await resp.text(), "yo");
    ac.abort();
    await server;
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerClose() {
  const ac = new AbortController();
  const server = Deno.serve(() => {}, { port: 4501, signal: ac.signal });
  const client = await Deno.connect({ port: 4501 });
  client.close();
  ac.abort();
  await server;
});

// FIXME:
// Deno.test(
//   { permissions: { net: true } },
//   async function httpServerEmptyBlobResponse() {
//     const ac = new AbortController();
//     const server = Deno.serve(() => new Response(new Blob([])), { port: 4501, signal: ac.signal });

//     const resp = await fetch("http://127.0.0.1:4501/");
//     const respBody = await resp.text();

//     assertEquals("", respBody);
//     ac.abort();
//     await server;
//   },
// );

// Deno.test({ permissions: { net: true } }, async function httpServerWebSocket() {
//   const ac = new AbortController();
//   const server = Deno.serve(async (request) => {
//     const {
//       response,
//       socket,
//     } = Deno.upgradeWebSocket(request);
//     socket.onerror = () => fail();
//     socket.onmessage = (m) => {
//       socket.send(m.data);
//       socket.close(1001);
//     };
//     return response;
//   }, { port: 4501, signal: ac.signal });

//   const def = deferred();
//   const ws = new WebSocket("ws://localhost:4501");
//   ws.onmessage = (m) => assertEquals(m.data, "foo");
//   ws.onerror = () => fail();
//   ws.onclose = () => def.resolve();
//   ws.onopen = () => ws.send("foo");
//   await def;
//   ac.abort();
//   await server;
// });

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequest() {
    const promise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve(async (request) => {
      headers = request.headers;
      promise.resolve();
      return new Response("");
    }, { port: 2333, signal: ac.signal });

    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const smthElse = "x".repeat(16 * 1024 + 256);
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\nSomething-Else: ${smthElse}\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();
    assertEquals(headers!.get("content-length"), "5");
    assertEquals(headers!.get("something-else"), smthElse);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequestAndBody() {
    const promise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    let text: string;
    const server = Deno.serve(async (request) => {
      headers = request.headers;
      text = await request.text();
      promise.resolve();
      return new Response("");
    }, { port: 2333, signal: ac.signal });

    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const smthElse = "x".repeat(16 * 1024 + 256);
    const reqBody = "hello world".repeat(1024);
    let body =
      `PUT / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: ${reqBody.length}\r\nSomething-Else: ${smthElse}\r\n\r\n${reqBody}`;

    while (body.length > 0) {
      const writeResult = await conn.write(encoder.encode(body));
      body = body.slice(writeResult);
    }

    await promise;
    conn.close();

    assertEquals(headers!.get("content-length"), `${reqBody.length}`);
    assertEquals(headers!.get("something-else"), smthElse);
    assertEquals(text!, reqBody);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpConnectionClose() {
    const promise = deferred();
    const ac = new AbortController();

    const server = Deno.serve(() => {
      promise.resolve();
      return new Response("");
    }, { port: 2333, signal: ac.signal });

    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + connection: close.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nConnection: Close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

// FIXME: auto request body reading is intefering with passing it as response.
// Deno.test(
//   { permissions: { net: true } },
//   async function httpServerStreamDuplex() {
//     const promise = deferred();
//     const ac = new AbortController();

//     const server = Deno.serve(request => {
//       assert(request.body);

//       promise.resolve();
//       return new Response(request.body);
//     }, { port: 2333, signal: ac.signal });

//     const ts = new TransformStream();
//     const writable = ts.writable.getWriter();

//     const resp = await fetch("http://127.0.0.1:2333/", {
//       method: "POST",
//       body: ts.readable,
//     });

//     await promise;
//     assert(resp.body);
//     const reader = resp.body.getReader();
//     await writable.write(new Uint8Array([1]));
//     const chunk1 = await reader.read();
//     assert(!chunk1.done);
//     assertEquals(chunk1.value, new Uint8Array([1]));
//     await writable.write(new Uint8Array([2]));
//     const chunk2 = await reader.read();
//     assert(!chunk2.done);
//     assertEquals(chunk2.value, new Uint8Array([2]));
//     await writable.close();
//     const chunk3 = await reader.read();
//     assert(chunk3.done);

//     ac.abort();
//     await server;
//   },
// );

Deno.test(
  { permissions: { net: true } },
  // Issue: https://github.com/denoland/deno/issues/10930
  async function httpServerStreamingResponse() {
    // This test enqueues a single chunk for readable
    // stream and waits for client to read that chunk and signal
    // it before enqueueing subsequent chunk. Issue linked above
    // presented a situation where enqueued chunks were not
    // written to the HTTP connection until the next chunk was enqueued.

    const promise = deferred();
    const ac = new AbortController();

    let counter = 0;

    const deferreds = [
      deferred(),
      deferred(),
      deferred(),
    ];

    async function writeRequest(conn: Deno.Conn) {
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();

      const w = new BufWriter(conn);
      const r = new BufReader(conn);
      const body = `GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\n\r\n`;
      const writeResult = await w.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await w.flush();
      const tpr = new TextProtoReader(r);
      const statusLine = await tpr.readLine();
      assert(statusLine !== null);
      const headers = await tpr.readMIMEHeader();
      assert(headers !== null);

      const chunkedReader = chunkedBodyReader(headers, r);

      const buf = new Uint8Array(5);
      const dest = new Buffer();

      let result: number | null;

      try {
        while ((result = await chunkedReader.read(buf)) !== null) {
          const len = Math.min(buf.byteLength, result);

          await dest.write(buf.subarray(0, len));

          // Resolve a deferred - this will make response stream to
          // enqueue next chunk.
          deferreds[counter - 1].resolve();
        }
        return decoder.decode(dest.bytes());
      } catch (e) {
        console.error(e);
      }
    }

    function periodicStream() {
      return new ReadableStream({
        start(controller) {
          controller.enqueue(`${counter}\n`);
          counter++;
        },

        async pull(controller) {
          if (counter >= 3) {
            return controller.close();
          }

          await deferreds[counter - 1];

          controller.enqueue(`${counter}\n`);
          counter++;
        },
      }).pipeThrough(new TextEncoderStream());
    }

    const finished = Deno.serve(() => {
      promise.resolve();
      return new Response(periodicStream());
    }, { port: 4501, signal: ac.signal });

    // start a client
    const clientConn = await Deno.connect({ port: 4501 });

    const r1 = await writeRequest(clientConn);

    assertEquals(r1, "0\n1\n2\n");

    ac.abort();
    await promise;
    await finished;
    clientConn.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpRequestLatin1Headers() {
    const promise = deferred();
    const ac = new AbortController();
    const server = Deno.serve((request) => {
      assertEquals(request.headers.get("X-Header-Test"), "á");
      promise.resolve();
      return new Response("hello", { headers: { "X-Header-Test": "Æ" } });
    }, { port: 4501, signal: ac.signal });

    const clientConn = await Deno.connect({ port: 4501 });
    const requestText =
      "GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\nX-Header-Test: á\r\n\r\n";
    const requestBytes = new Uint8Array(requestText.length);
    for (let i = 0; i < requestText.length; i++) {
      requestBytes[i] = requestText.charCodeAt(i);
    }
    let written = 0;
    while (written < requestBytes.byteLength) {
      written += await clientConn.write(requestBytes.slice(written));
    }    

    const buf = new Uint8Array(1024);
    assertEquals(await clientConn.read(buf), 79);
    await promise;
    let responseText = new TextDecoder().decode(buf);  
    clientConn.close();

    
    assert(/\r\n[Xx]-[Hh]eader-[Tt]est: Æ\r\n/.test(responseText));

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerRequestWithoutPath() {
    const promise = deferred();
    const ac = new AbortController();

    const server = Deno.serve(async (request) => {
      // FIXME:
      // assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
      assertEquals(await request.text(), "");
      promise.resolve();
      return new Response("11");
    }, { port: 4501, signal: ac.signal });

    const clientConn = await Deno.connect({ port: 4501 });

    async function writeRequest(conn: Deno.Conn) {
      const encoder = new TextEncoder();

      const w = new BufWriter(conn);
      const r = new BufReader(conn);
      const body =
        `CONNECT 127.0.0.1:4501 HTTP/1.1\r\nHost: 127.0.0.1:4501\r\n\r\n`;
      const writeResult = await w.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await w.flush();
      const tpr = new TextProtoReader(r);
      const statusLine = await tpr.readLine();
      assert(statusLine !== null);
      const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
      assert(m !== null, "must be matched");
      const [_, _proto, status, _ok] = m;
      assertEquals(status, "200");
      const headers = await tpr.readMIMEHeader();
      assert(headers !== null);
    }

    await writeRequest(clientConn);
    clientConn.close();
    await promise;

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpCookieConcatenation() {
    const promise = deferred();
    const ac = new AbortController();

    const server = Deno.serve(async (request) => {
      assertEquals(await request.text(), "");
      assertEquals(request.headers.get("cookie"), "foo=bar, bar=foo");
      promise.resolve();
      return new Response("ok");
    }, { port: 4501, signal: ac.signal });

    const resp = await fetch("http://127.0.0.1:4501/", {
      headers: [
        ["connection", "close"],
        ["cookie", "foo=bar"],
        ["cookie", "bar=foo"],
      ],
    });
    await promise;

    const text = await resp.text();
    assertEquals(text, "ok");

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerCorrectSizeResponse() {
    const promise = deferred();
    const ac = new AbortController();

    const tmpFile = await Deno.makeTempFile();
    const file = await Deno.open(tmpFile, { write: true, read: true });
    await file.write(new Uint8Array(70 * 1024).fill(1)); // 70kb sent in 64kb + 6kb chunks
    file.close();
    
    const server = Deno.serve(async (request) => {
      const f = await Deno.open(tmpFile, { read: true });
      promise.resolve();
      return new Response(f.readable);
    }, { port: 4503, signal: ac.signal });

    const resp = await fetch("http://127.0.0.1:4503/");
    await promise;
    const body = await resp.arrayBuffer();
    
    assertEquals(body.byteLength, 70 * 1024);
    ac.abort();
    await server;
  },
);

// https://github.com/denoland/deno/issues/12741
// https://github.com/denoland/deno/pull/12746
// https://github.com/denoland/deno/pull/12798
Deno.test(
  { permissions: { net: true, run: true } },
  async function httpServerDeleteRequestHasBody() {
    const promise = deferred();
    const ac = new AbortController();

    const hostname = "localhost";
    const port = 4501;

    const server = Deno.serve(() => {
      promise.resolve();
      return new Response("ok");      
    }, { port: port, signal: ac.signal });

    const url = `http://${hostname}:${port}/`;
    const args = ["-X", "DELETE", url];
    const { success } = await Deno.spawn("curl", {
      args,
      stdout: "null",
      stderr: "null",
    });
    assert(success);
    await promise;
    ac.abort();

    await server;
  },
);

function chunkedBodyReader(h: Headers, r: BufReader): Deno.Reader {
  // Based on https://tools.ietf.org/html/rfc2616#section-19.4.6
  const tp = new TextProtoReader(r);
  let finished = false;
  const chunks: Array<{
    offset: number;
    data: Uint8Array;
  }> = [];
  async function read(buf: Uint8Array): Promise<number | null> {
    if (finished) return null;
    const [chunk] = chunks;
    if (chunk) {
      const chunkRemaining = chunk.data.byteLength - chunk.offset;
      const readLength = Math.min(chunkRemaining, buf.byteLength);
      for (let i = 0; i < readLength; i++) {
        buf[i] = chunk.data[chunk.offset + i];
      }
      chunk.offset += readLength;
      if (chunk.offset === chunk.data.byteLength) {
        chunks.shift();
        // Consume \r\n;
        if ((await tp.readLine()) === null) {
          throw new Deno.errors.UnexpectedEof();
        }
      }
      return readLength;
    }
    const line = await tp.readLine();
    if (line === null) throw new Deno.errors.UnexpectedEof();
    // TODO(bartlomieju): handle chunk extension
    const [chunkSizeString] = line.split(";");
    const chunkSize = parseInt(chunkSizeString, 16);
    if (Number.isNaN(chunkSize) || chunkSize < 0) {
      throw new Deno.errors.InvalidData("Invalid chunk size");
    }
    if (chunkSize > 0) {
      if (chunkSize > buf.byteLength) {
        let eof = await r.readFull(buf);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        const restChunk = new Uint8Array(chunkSize - buf.byteLength);
        eof = await r.readFull(restChunk);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        } else {
          chunks.push({
            offset: 0,
            data: restChunk,
          });
        }
        return buf.byteLength;
      } else {
        const bufToFill = buf.subarray(0, chunkSize);
        const eof = await r.readFull(bufToFill);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        // Consume \r\n
        if ((await tp.readLine()) === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        return chunkSize;
      }
    } else {
      assert(chunkSize === 0);
      // Consume \r\n
      if ((await r.readLine()) === null) {
        throw new Deno.errors.UnexpectedEof();
      }
      await readTrailers(h, r);
      finished = true;
      return null;
    }
  }
  return { read };
}

async function readTrailers(
  headers: Headers,
  r: BufReader,
) {
  const trailers = parseTrailer(headers.get("trailer"));
  if (trailers == null) return;
  const trailerNames = [...trailers.keys()];
  const tp = new TextProtoReader(r);
  const result = await tp.readMIMEHeader();
  if (result == null) {
    throw new Deno.errors.InvalidData("Missing trailer header.");
  }
  const undeclared = [...result.keys()].filter(
    (k) => !trailerNames.includes(k),
  );
  if (undeclared.length > 0) {
    throw new Deno.errors.InvalidData(
      `Undeclared trailers: ${Deno.inspect(undeclared)}.`,
    );
  }
  for (const [k, v] of result) {
    headers.append(k, v);
  }
  const missingTrailers = trailerNames.filter((k) => !result.has(k));
  if (missingTrailers.length > 0) {
    throw new Deno.errors.InvalidData(
      `Missing trailers: ${Deno.inspect(missingTrailers)}.`,
    );
  }
  headers.delete("trailer");
}

function parseTrailer(field: string | null): Headers | undefined {
  if (field == null) {
    return undefined;
  }
  const trailerNames = field.split(",").map((v) => v.trim().toLowerCase());
  if (trailerNames.length === 0) {
    throw new Deno.errors.InvalidData("Empty trailer header.");
  }
  const prohibited = trailerNames.filter((k) => isProhibitedForTrailer(k));
  if (prohibited.length > 0) {
    throw new Deno.errors.InvalidData(
      `Prohibited trailer names: ${Deno.inspect(prohibited)}.`,
    );
  }
  return new Headers(trailerNames.map((key) => [key, ""]));
}
