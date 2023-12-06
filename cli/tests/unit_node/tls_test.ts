// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assertEquals,
  assertInstanceOf,
} from "../../../test_util/std/assert/mod.ts";
import { delay } from "../../../test_util/std/async/delay.ts";
import { fromFileUrl, join } from "../../../test_util/std/path/mod.ts";
import { serveTls } from "../../../test_util/std/http/server.ts";
import * as tls from "node:tls";
import * as net from "node:net";
import * as stream from "node:stream";

const tlsTestdataDir = fromFileUrl(
  new URL("../testdata/tls", import.meta.url),
);
const keyFile = join(tlsTestdataDir, "localhost.key");
const certFile = join(tlsTestdataDir, "localhost.crt");
const key = await Deno.readTextFile(keyFile);
const cert = await Deno.readTextFile(certFile);
const rootCaCert = await Deno.readTextFile(join(tlsTestdataDir, "RootCA.pem"));

Deno.test("tls.connect makes tls connection", async () => {
  const ctl = new AbortController();
  const serve = serveTls(() => new Response("hello"), {
    port: 8443,
    key,
    cert,
    signal: ctl.signal,
  });

  await delay(200);

  const conn = tls.connect({
    host: "localhost",
    port: 8443,
    secureContext: {
      ca: rootCaCert,
      // deno-lint-ignore no-explicit-any
    } as any,
  });
  conn.write(`GET / HTTP/1.1
Host: localhost
Connection: close

`);
  conn.on("data", (chunk) => {
    const text = new TextDecoder().decode(chunk);
    const bodyText = text.split("\r\n\r\n").at(-1)?.trim();
    assertEquals(bodyText, "hello");
    conn.destroy();
    ctl.abort();
  });

  await serve;
});

// https://github.com/denoland/deno/pull/20120
Deno.test("tls.connect mid-read tcp->tls upgrade", async () => {
  const ctl = new AbortController();
  const serve = serveTls(() => new Response("hello"), {
    port: 8443,
    key,
    cert,
    signal: ctl.signal,
  });

  await delay(200);

  const conn = tls.connect({
    host: "localhost",
    port: 8443,
    secureContext: {
      ca: rootCaCert,
      // deno-lint-ignore no-explicit-any
    } as any,
  });

  conn.setEncoding("utf8");
  conn.write(`GET / HTTP/1.1\nHost: www.google.com\n\n`);

  conn.on("data", (_) => {
    conn.destroy();
    ctl.abort();
  });

  await serve;
});

Deno.test("tls.createServer creates a TLS server", async () => {
  const deferred = Promise.withResolvers<void>();
  const server = tls.createServer(
    // deno-lint-ignore no-explicit-any
    { host: "0.0.0.0", key, cert } as any,
    (socket: net.Socket) => {
      socket.write("welcome!\n");
      socket.setEncoding("utf8");
      socket.pipe(socket).on("data", (data) => {
        if (data.toString().trim() === "goodbye") {
          socket.destroy();
        }
      });
    },
  );
  server.listen(0, async () => {
    const conn = await Deno.connectTls({
      hostname: "127.0.0.1",
      // deno-lint-ignore no-explicit-any
      port: (server.address() as any).port,
      caCerts: [rootCaCert],
    });

    const buf = new Uint8Array(100);
    await Deno.read(conn.rid, buf);
    let text: string;
    text = new TextDecoder().decode(buf);
    assertEquals(text.replaceAll("\0", ""), "welcome!\n");
    buf.fill(0);

    Deno.write(conn.rid, new TextEncoder().encode("hey\n"));
    await Deno.read(conn.rid, buf);
    text = new TextDecoder().decode(buf);
    assertEquals(text.replaceAll("\0", ""), "hey\n");
    buf.fill(0);

    Deno.write(conn.rid, new TextEncoder().encode("goodbye\n"));
    await Deno.read(conn.rid, buf);
    text = new TextDecoder().decode(buf);
    assertEquals(text.replaceAll("\0", ""), "goodbye\n");

    conn.close();
    server.close();
    deferred.resolve();
  });
  await deferred.promise;
});

Deno.test("TLSSocket can construct without options", () => {
  // deno-lint-ignore no-explicit-any
  new tls.TLSSocket(new stream.PassThrough() as any);
});

Deno.test("tlssocket._handle._parentWrap is set", () => {
  // Note: This feature is used in popular 'http2-wrapper' module
  // https://github.com/szmarczak/http2-wrapper/blob/51eeaf59ff9344fb192b092241bfda8506983620/source/utils/js-stream-socket.js#L6
  const parentWrap =
    // deno-lint-ignore no-explicit-any
    ((new tls.TLSSocket(new stream.PassThrough() as any, {}) as any)
      // deno-lint-ignore no-explicit-any
      ._handle as any)!
      ._parentWrap;
  assertInstanceOf(parentWrap, stream.PassThrough);
});
