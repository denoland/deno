// Copyright 2018-2025 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertStringIncludes,
  assertThrows,
} from "@std/assert";
import { delay } from "@std/async/delay";
import { dirname, fromFileUrl, join } from "@std/path";
import * as tls from "node:tls";
import * as net from "node:net";
import * as stream from "node:stream";
import { execCode } from "../unit/test_util.ts";

const tlsTestdataDir = fromFileUrl(
  new URL("../testdata/tls", import.meta.url),
);
const key = Deno.readTextFileSync(join(tlsTestdataDir, "localhost.key"));
const cert = Deno.readTextFileSync(join(tlsTestdataDir, "localhost.crt"));
const rootCaCert = Deno.readTextFileSync(join(tlsTestdataDir, "RootCA.pem"));

for (
  const [alpnServer, alpnClient, expected] of [
    [["a", "b"], ["a"], ["a"]],
    [["a", "b"], ["b"], ["b"]],
    [["a", "b"], ["a", "b"], ["a"]],
    [["a", "b"], [], []],
    [[], ["a", "b"], []],
  ]
) {
  Deno.test(`tls.connect sends correct ALPN: '${alpnServer}' + '${alpnClient}' = '${expected}'`, async () => {
    const listener = Deno.listenTls({
      port: 0,
      key,
      cert,
      alpnProtocols: alpnServer,
    });
    const outgoing = tls.connect({
      host: "localhost",
      port: listener.addr.port,
      ALPNProtocols: alpnClient,
      secureContext: {
        ca: rootCaCert,
        // deno-lint-ignore no-explicit-any
      } as any,
    });

    const conn = await listener.accept();
    const handshake = await conn.handshake();
    assertEquals(handshake.alpnProtocol, expected[0] || null);
    conn.close();
    outgoing.destroy();
    listener.close();
    await new Promise((resolve) => outgoing.on("close", resolve));
  });
}

Deno.test("tls.connect makes tls connection", async () => {
  const ctl = new AbortController();
  let port;
  const serve = Deno.serve({
    port: 0,
    key,
    cert,
    signal: ctl.signal,
    onListen: (listen) => port = listen.port,
  }, () => new Response("hello"));

  await delay(200);

  const conn = tls.connect({
    port,
    secureContext: {
      ca: rootCaCert,
      // deno-lint-ignore no-explicit-any
    } as any,
  });
  conn.write(`GET / HTTP/1.1
Host: localhost
Connection: close

`);
  const chunk = Promise.withResolvers<Uint8Array>();
  conn.on("data", (received) => {
    conn.destroy();
    ctl.abort();
    chunk.resolve(received);
  });

  await serve.finished;

  const text = new TextDecoder().decode(await chunk.promise);
  const bodyText = text.split("\r\n\r\n").at(-1)?.trim();
  assertEquals(bodyText, "hello");
});

// https://github.com/denoland/deno/pull/20120
Deno.test("tls.connect mid-read tcp->tls upgrade", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ctl = new AbortController();
  const serve = Deno.serve({
    port: 8443,
    key,
    cert,
    signal: ctl.signal,
  }, () => new Response("hello"));

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
  conn.on("close", resolve);

  await serve.finished;
  await promise;
});

Deno.test("tls.connect after-read tls upgrade", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ctl = new AbortController();
  const serve = Deno.serve({
    port: 8444,
    key,
    cert,
    signal: ctl.signal,
  }, () => new Response("hello"));

  await delay(200);

  const socket = net.connect({
    host: "localhost",
    port: 8444,
  });
  socket.on("connect", () => {
    socket.on("data", () => {});
    socket.on("close", resolve);

    socket.removeAllListeners("data");

    const conn = tls.connect({
      host: "localhost",
      port: 8444,
      socket,
      secureContext: {
        ca: rootCaCert,
        key: null,
        cert: null,
        // deno-lint-ignore no-explicit-any
      } as any,
    });

    conn.setEncoding("utf8");
    conn.write(`GET / HTTP/1.1\nHost: www.google.com\n\n`);

    conn.on("data", (e) => {
      assertStringIncludes(e, "hello");
      conn.destroy();
      ctl.abort();
    });
  });

  await serve.finished;
  await promise;
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
      socket.on("close", () => deferred.resolve());
    },
  );
  server.listen(0, async () => {
    const tcpConn = await Deno.connect({
      // deno-lint-ignore no-explicit-any
      port: (server.address() as any).port,
    });
    const conn = await Deno.startTls(tcpConn, {
      hostname: "localhost",
      caCerts: [rootCaCert],
    });

    const buf = new Uint8Array(100);
    await conn.read(buf);
    let text: string;
    text = new TextDecoder().decode(buf);
    assertEquals(text.replaceAll("\0", ""), "welcome!\n");
    buf.fill(0);

    await conn.write(new TextEncoder().encode("hey\n"));
    await conn.read(buf);
    text = new TextDecoder().decode(buf);
    assertEquals(text.replaceAll("\0", ""), "hey\n");
    buf.fill(0);

    await conn.write(new TextEncoder().encode("goodbye\n"));
    await conn.read(buf);
    text = new TextDecoder().decode(buf);
    assertEquals(text.replaceAll("\0", ""), "goodbye\n");

    conn.close();
    server.close();
  });
  await deferred.promise;
});

Deno.test("TLSSocket can construct without options", () => {
  // deno-lint-ignore no-explicit-any
  new tls.TLSSocket(new stream.PassThrough() as any);
});

Deno.test("tls.connect() throws InvalidData when there's error in certificate", async () => {
  // Uses execCode to avoid `--unsafely-ignore-certificate-errors` option applied
  const [status, output] = await execCode(`
    import tls from "node:tls";
    const conn = tls.connect({
      host: "localhost",
      port: 4557,
    });

    conn.on("error", (err) => {
      console.log(err);
    });
  `);

  assertEquals(status, 0);
  assertStringIncludes(
    output,
    "InvalidData: invalid peer certificate: UnknownIssuer",
  );
});

Deno.test("tls.rootCertificates is not empty", () => {
  assert(tls.rootCertificates.length > 0);
  assert(Object.isFrozen(tls.rootCertificates));
  assert(tls.rootCertificates instanceof Array);
  assert(tls.rootCertificates.every((cert) => typeof cert === "string"));
  assertThrows(() => {
    (tls.rootCertificates as string[]).push("new cert");
  }, TypeError);
});

Deno.test("TLSSocket.alpnProtocol is set for client", async () => {
  const listener = Deno.listenTls({
    hostname: "localhost",
    port: 0,
    key,
    cert,
    alpnProtocols: ["a"],
  });
  const outgoing = tls.connect({
    host: "::1",
    servername: "localhost",
    port: listener.addr.port,
    ALPNProtocols: ["a"],
    secureContext: {
      ca: rootCaCert,
      // deno-lint-ignore no-explicit-any
    } as any,
  });

  const conn = await listener.accept();
  const handshake = await conn.handshake();
  assertEquals(handshake.alpnProtocol, "a");
  conn.close();
  outgoing.destroy();
  listener.close();
  await new Promise((resolve) => outgoing.on("close", resolve));
});

Deno.test("tls connect upgrade tcp", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const socket = new net.Socket();
  socket.connect(443, "google.com");
  socket.on("connect", () => {
    const secure = tls.connect({ socket });
    secure.on("secureConnect", () => resolve());
  });

  await promise;
  socket.destroy();
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

Deno.test({
  name: "tls connect upgrade js socket wrapper",
  sanitizeOps: false,
  sanitizeResources: false,
}, async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  class SocketWrapper extends stream.Duplex {
    socket: net.Socket;

    constructor() {
      super();
      this.socket = new net.Socket();
    }

    // deno-lint-ignore no-explicit-any
    override _write(chunk: any, encoding: any, callback: any) {
      this.socket.write(chunk, encoding, callback);
    }

    override _read() {
    }

    connect(port: number, host: string) {
      this.socket.connect(port, host);
      this.socket.on("data", (data) => this.push(data));
      this.socket.on("end", () => this.push(null));
    }
  }

  const socket = new SocketWrapper();
  socket.connect(443, "google.com");

  const secure = tls.connect({ socket, host: "google.com" });
  secure.on("secureConnect", () => resolve());

  await promise;
  socket.destroy();
});

Deno.test({
  name: "[node/tls] tls.Server.unref() works",
  ignore: Deno.build.os === "windows",
}, async () => {
  const { stdout, stderr } = await new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      `
        import * as tls from "node:tls";
        
        const key = Deno.readTextFileSync("${
        join(tlsTestdataDir, "localhost.key")
      }");
        const cert = Deno.readTextFileSync("${
        join(tlsTestdataDir, "localhost.crt")
      }");
        
        const server = tls.createServer({ key, cert }, (socket) => {
          socket.end("hello\\n");
        });

        server.unref();
        server.listen(0, () => {});
      `,
    ],
    cwd: dirname(fromFileUrl(import.meta.url)),
  }).output();

  if (stderr.length > 0) {
    throw new Error(`stderr: ${new TextDecoder().decode(stderr)}`);
  }
  assertEquals(new TextDecoder().decode(stdout), "");
});

Deno.test("mTLS client certificate authentication", async () => {
  const clientKey = key;
  const clientCert = cert;

  const server = tls.createServer({
    key,
    cert,
    requestCert: true,
    rejectUnauthorized: true,
    ca: [rootCaCert],
  }, (socket) => {
    socket.write("mTLS success!");
    socket.end();
  });

  const { promise, resolve, reject } = Promise.withResolvers<string>();

  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any)?.port;

    const client = tls.connect({
      host: "localhost",
      port,
      key: clientKey,
      cert: clientCert,
      ca: rootCaCert,
    });

    client.setEncoding("utf8");
    let data = "";
    client.on("data", (chunk) => {
      data += chunk;
    });

    client.on("end", () => {
      resolve(data);
    });

    client.on("error", (err) => {
      reject(err);
    });
  });

  const result = await promise;
  assertEquals(result, "mTLS success!");
  server.close();
});
