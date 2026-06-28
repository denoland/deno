// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertMatch,
  assertStringIncludes,
  assertThrows,
} from "@std/assert";
import { delay } from "@std/async/delay";
import { dirname, fromFileUrl, join } from "@std/path";
import * as tls from "node:tls";
import * as net from "node:net";
import * as stream from "node:stream";
import { setImmediate } from "node:timers";
import { Buffer } from "node:buffer";
import { execCode } from "../unit/test_util.ts";

const tlsTestdataDir = fromFileUrl(
  new URL("../testdata/tls", import.meta.url),
);
const key = Deno.readTextFileSync(join(tlsTestdataDir, "localhost.key"));
const cert = Deno.readTextFileSync(join(tlsTestdataDir, "localhost.crt"));
const rootCaCert = Deno.readTextFileSync(join(tlsTestdataDir, "RootCA.pem"));

// Regression test for https://github.com/denoland/deno/issues/30724
// TLS over a back-to-back Duplex pair (like native-duplexpair used by
// tedious/mssql) previously panicked with "RefCell already borrowed"
// because encOut synchronously wrote to the paired stream, re-entering
// the same CppGC RefCell.
Deno.test("tls over js-backed duplex pair does not panic", async () => {
  const server = tls.createServer({ cert, key }, (socket) => {
    socket.on("error", () => {});
    socket.write("hello from server");
    socket.end();
  });

  const { promise: listening, resolve: resolveListening } = Promise
    .withResolvers<void>();
  server.listen(0, () => resolveListening());
  await listening;
  const { port } = server.address() as net.AddressInfo;

  // Raw TCP connection to the TLS server.
  const rawSocket = net.connect(port, "localhost");
  const { promise: connected, resolve: resolveConnected } = Promise
    .withResolvers<void>();
  rawSocket.on("connect", () => resolveConnected());
  await connected;

  // Wrap rawSocket in a plain Duplex (NOT a net.Socket) to trigger
  // JSStreamSocket in _tls_wrap.js, mimicking tedious/mssql TLS-over-TDS.
  const wrapper = new stream.Duplex({
    read() {},
    write(
      chunk: Uint8Array,
      _enc: string,
      cb: (err?: Error | null) => void,
    ) {
      if (rawSocket.destroyed) {
        cb();
        return;
      }
      rawSocket.write(chunk, cb);
    },
  });
  rawSocket.on("data", (d: Uint8Array) => wrapper.push(d));
  rawSocket.on("end", () => wrapper.push(null));

  const tlsSocket = tls.connect({
    socket: wrapper as net.Socket,
    rejectUnauthorized: false,
  });

  const received = await new Promise<string>((resolve, reject) => {
    let data = "";
    tlsSocket.on("error", reject);
    tlsSocket.on("data", (chunk: Uint8Array) => {
      data += chunk.toString();
    });
    tlsSocket.on("end", () => resolve(data));
  });

  assertEquals(received, "hello from server");

  tlsSocket.destroy();
  rawSocket.destroy();
  server.close();
});

// Back-to-back Duplex pair (mimics native-duplexpair used by tedious/mssql):
// bytes written to one end surface as "data" on the other. A TLSSocket over
// one end (a plain Duplex, not a net.Socket) takes the JSStreamSocket path.
function backToBackDuplexPair() {
  const socket1 = new stream.Duplex({
    read() {},
    write(chunk: Uint8Array, _enc: string, cb: () => void) {
      socket2.push(chunk);
      cb();
    },
    final(cb: () => void) {
      socket2.push(null);
      cb();
    },
  });
  const socket2 = new stream.Duplex({
    read() {},
    write(chunk: Uint8Array, _enc: string, cb: () => void) {
      socket1.push(chunk);
      cb();
    },
    final(cb: () => void) {
      socket1.push(null);
      cb();
    },
  });
  return { socket1, socket2 };
}

// Wrap a raw socket's transport in a TLSSocket that runs over a back-to-back
// Duplex pair (the JSStreamSocket path). Both peers in the tests below use this
// so close_notify propagation is actually exercised: a native peer would still
// observe the TCP FIN even when the close_notify is dropped, hiding the bug.
function wrapJsBackedTls(
  raw: net.Socket,
  // deno-lint-ignore no-explicit-any
  options: any,
): tls.TLSSocket {
  const { socket1, socket2 } = backToBackDuplexPair();
  raw.pipe(socket2);
  socket2.pipe(raw);
  raw.on("error", () => {});
  const sock = options.isServer
    ? new tls.TLSSocket(socket1 as net.Socket, options)
    : tls.connect({ socket: socket1 as net.Socket, ...options });
  sock.on("error", () => {});
  return sock;
}

// Regression test: a server-side TLSSocket over a JS-backed Duplex pair
// (JSStreamSocket path, like tedious/mssql TLS-over-TDS) must send the TLS
// close_notify and end the underlying stream when `.end()` is called. Otherwise
// the peer never observes EOF and hangs (surfaces in tedious/Prisma as
// "Connection lost - socket hang up").
Deno.test("tls js-backed duplex server propagates close_notify on end()", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<string>();
  let serverTls: tls.TLSSocket | undefined;
  let serverRaw: net.Socket | undefined;
  let clientTls: tls.TLSSocket | undefined;
  let clientRaw: net.Socket | undefined;

  const server = net.createServer((raw: net.Socket) => {
    serverRaw = raw;
    serverTls = wrapJsBackedTls(raw, {
      isServer: true,
      key,
      cert,
      maxVersion: "TLSv1.2",
    });
    serverTls.on("secure", () => {
      serverTls!.write("hello from server");
      // Close on a later tick, so the connection is idle when `.end()` runs
      // (the pooled-connection pattern). Ending synchronously here would let
      // the close_notify ride the write's flush and mask the bug.
      setImmediate(() => serverTls!.end());
    });
  });

  server.listen(0, () => {
    const { port } = server.address() as net.AddressInfo;
    clientRaw = net.connect(port, "localhost", () => {
      clientTls = wrapJsBackedTls(clientRaw!, {
        servername: "localhost",
        rejectUnauthorized: false,
        maxVersion: "TLSv1.2",
      });
      let data = "";
      clientTls.on("data", (chunk: Uint8Array) => {
        data += chunk.toString();
      });
      // If close_notify/EOF is not propagated, "end" never fires and the test
      // times out via the harness (the regression this guards against).
      clientTls.on("end", () => resolve(data));
    });
    clientRaw.on("error", reject);
  });

  const received = await promise;
  assertEquals(received, "hello from server");
  clientTls?.destroy();
  serverTls?.destroy();
  clientRaw?.destroy();
  serverRaw?.destroy();
  server.close();
});

// Symmetric to the above: a client-side TLSSocket over a JS-backed Duplex pair
// must also propagate close_notify/EOF on `.end()` so the peer sees the end.
Deno.test("tls js-backed duplex client propagates close_notify on end()", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  let serverTls: tls.TLSSocket | undefined;
  let serverRaw: net.Socket | undefined;
  let clientTls: tls.TLSSocket | undefined;
  let clientRaw: net.Socket | undefined;

  const server = net.createServer((raw: net.Socket) => {
    serverRaw = raw;
    serverTls = wrapJsBackedTls(raw, {
      isServer: true,
      key,
      cert,
      maxVersion: "TLSv1.2",
    });
    serverTls.resume();
    // The client's close_notify/EOF must surface here as "end".
    serverTls.on("end", () => resolve());
  });

  server.listen(0, () => {
    const { port } = server.address() as net.AddressInfo;
    clientRaw = net.connect(port, "localhost", () => {
      clientTls = wrapJsBackedTls(clientRaw!, {
        servername: "localhost",
        rejectUnauthorized: false,
        maxVersion: "TLSv1.2",
      });
      clientTls.on("secureConnect", () => {
        clientTls!.write("hello from client");
        // Close on a later tick (idle connection); see the server-side test.
        setImmediate(() => clientTls!.end());
      });
    });
    clientRaw.on("error", reject);
  });

  await promise;
  clientTls?.destroy();
  serverTls?.destroy();
  clientRaw?.destroy();
  serverRaw?.destroy();
  server.close();
});

// `.end()` called before the handshake completes: the native shutdown defers
// the close_notify until the handshake finishes, so the underlying stream must
// only be ended afterwards. Ending it eagerly would tear the transport down
// mid-handshake and the peer would never see EOF.
Deno.test("tls js-backed duplex client end() during handshake still sends close_notify", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  let sawSecure = false;
  let serverTls: tls.TLSSocket | undefined;
  let serverRaw: net.Socket | undefined;
  let clientTls: tls.TLSSocket | undefined;
  let clientRaw: net.Socket | undefined;

  const server = net.createServer((raw: net.Socket) => {
    serverRaw = raw;
    serverTls = wrapJsBackedTls(raw, {
      isServer: true,
      key,
      cert,
      maxVersion: "TLSv1.2",
    });
    serverTls.resume();
    // Reached only if the handshake completed and the deferred close_notify
    // was produced and flushed (not if the transport was torn down early).
    serverTls.on("end", () => resolve());
  });

  server.listen(0, () => {
    const { port } = server.address() as net.AddressInfo;
    clientRaw = net.connect(port, "localhost", () => {
      clientTls = wrapJsBackedTls(clientRaw!, {
        servername: "localhost",
        rejectUnauthorized: false,
        maxVersion: "TLSv1.2",
      });
      clientTls.on("secureConnect", () => {
        sawSecure = true;
      });
      // Ends before the handshake can complete, exercising the deferred path.
      clientTls.end();
    });
    clientRaw.on("error", reject);
  });

  await promise;
  assert(sawSecure, "handshake should complete before EOF is propagated");
  clientTls?.destroy();
  serverTls?.destroy();
  clientRaw?.destroy();
  serverRaw?.destroy();
  server.close();
});

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

Deno.test(
  { name: "tls.connect after-read tls upgrade" },
  async () => {
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
  },
);

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
  await new Promise<void>((resolve) => server.on("close", resolve));
});

Deno.test("TLSSocket can construct without options", () => {
  // deno-lint-ignore no-explicit-any
  new tls.TLSSocket(new stream.PassThrough() as any);
});

// Regression test for https://github.com/denoland/deno/issues/33743
// `setServername` must throw with Node's `code` property set, not a plain
// `TypeError`/`Error`.
Deno.test("TLSSocket.setServername - throws ERR_INVALID_ARG_TYPE for non-string", () => {
  // deno-lint-ignore no-explicit-any
  const sock: any = new tls.TLSSocket(new stream.PassThrough() as any);
  const err = assertThrows(() => sock.setServername(123), TypeError);
  assertEquals((err as { code?: string }).code, "ERR_INVALID_ARG_TYPE");
});

Deno.test("TLSSocket.setServername - throws ERR_TLS_SNI_FROM_SERVER on server-side socket", () => {
  // deno-lint-ignore no-explicit-any
  const sock: any = new tls.TLSSocket(new stream.PassThrough() as any, {
    isServer: true,
  });
  const err = assertThrows(() => sock.setServername("example.com"));
  assertEquals((err as { code?: string }).code, "ERR_TLS_SNI_FROM_SERVER");
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
    "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
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
    hostname: "::1",
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

Deno.test({ name: "tls connect upgrade tcp" }, async () => {
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
  // _parentWrap is a JSStreamSocket wrapping the PassThrough (since
  // PassThrough is not a net.Socket, TLSSocket wraps it in JSStreamSocket).
  assert(parentWrap != null);
});

Deno.test("net.Socket reinitialize preserves TLS upgrade state", () => {
  const socket = new net.Socket();
  const reinitializeHandle = Object.getOwnPropertySymbols(net.Socket.prototype)
    .find((symbol) => symbol.description === "kReinitializeHandle");

  assert(reinitializeHandle, "expected kReinitializeHandle symbol");
  const reinitializeHandleSymbol = reinitializeHandle as symbol;

  let closed = false;
  const afterConnectTls = function () {};
  const verifyError = () => null;
  const parentWrap = new stream.PassThrough();

  // deno-lint-ignore no-explicit-any
  (socket as any)._handle = {
    close() {
      closed = true;
    },
    afterConnectTls,
    verifyError,
    _parentWrap: parentWrap,
  };

  const newHandle = {};
  // deno-lint-ignore no-explicit-any
  (socket as any)[reinitializeHandleSymbol](newHandle);

  assert(closed);
  // deno-lint-ignore no-explicit-any
  assertEquals((newHandle as any).afterConnectTls, afterConnectTls);
  // deno-lint-ignore no-explicit-any
  assertEquals(typeof (newHandle as any).afterConnectTlsResolve, "function");
  // deno-lint-ignore no-explicit-any
  assert((newHandle as any).upgrading instanceof Promise);
  // deno-lint-ignore no-explicit-any
  assertEquals((newHandle as any).verifyError, verifyError);
  // deno-lint-ignore no-explicit-any
  assertEquals((newHandle as any)._parent, newHandle);
  // deno-lint-ignore no-explicit-any
  assertEquals((newHandle as any)._parentWrap, parentWrap);
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
      client.destroy();
      resolve(data);
    });

    client.on("error", (err) => {
      reject(err);
    });
  });

  const result = await promise;
  assertEquals(result, "mTLS success!");
  server.close();
  await new Promise<void>((resolve) => server.on("close", resolve));
});

Deno.test(
  "requestCert + rejectUnauthorized:false: no client cert => authorized=false",
  async () => {
    const server = tls.createServer({
      key,
      cert,
      ca: [rootCaCert],
      requestCert: true,
      rejectUnauthorized: false,
    }, (socket) => {
      // deno-lint-ignore no-explicit-any
      const s = socket as any;
      socket.write(
        JSON.stringify({
          authorized: s.authorized,
          authorizationError: s.authorizationError?.code ??
            s.authorizationError,
          peerCertSubject: socket.getPeerCertificate()?.subject,
        }),
      );
      socket.end();
    });

    const { promise, resolve, reject } = Promise.withResolvers<string>();

    server.listen(0, () => {
      // deno-lint-ignore no-explicit-any
      const port = (server.address() as any)?.port;

      const client = tls.connect({
        host: "localhost",
        port,
        ca: rootCaCert,
      });

      client.setEncoding("utf8");
      let data = "";
      client.on("data", (chunk) => {
        data += chunk;
      });
      client.on("end", () => {
        client.destroy();
        resolve(data);
      });
      client.on("error", (err) => reject(err));
    });

    const result = JSON.parse(await promise);
    assertEquals(result.authorized, false);
    assertEquals(result.authorizationError, "UNABLE_TO_GET_ISSUER_CERT");
    assertEquals(result.peerCertSubject, undefined);
    server.close();
    await new Promise<void>((resolve) => server.on("close", resolve));
  },
);

Deno.test(
  "tls PFX: cert+key from pfx are used for handshake",
  async () => {
    // Regression test for https://github.com/denoland/deno/issues/34202:
    // the cert/key embedded in PFX must be extracted into the SecureContext
    // so the TLS handshake doesn't fail with no-server-cert.
    const pfx = Buffer.from(
      Deno.readFileSync(join(tlsTestdataDir, "localhost.pfx")),
    );

    const server = tls.createServer({
      pfx,
      passphrase: "testpass",
      requestCert: true,
      rejectUnauthorized: false,
    }, (socket) => {
      // deno-lint-ignore no-explicit-any
      const s = socket as any;
      socket.write(JSON.stringify({
        authorized: s.authorized,
        authorizationError: s.authorizationError?.code ?? s.authorizationError,
      }));
      socket.end();
    });

    const { promise, resolve, reject } = Promise.withResolvers<string>();

    server.listen(0, () => {
      // deno-lint-ignore no-explicit-any
      const port = (server.address() as any)?.port;
      const client = tls.connect({
        host: "localhost",
        port,
        pfx,
        passphrase: "testpass",
        rejectUnauthorized: false,
      });
      client.setEncoding("utf8");
      let data = "";
      client.on("data", (chunk) => {
        data += chunk;
      });
      client.on("end", () => {
        // deno-lint-ignore no-explicit-any
        const ce = (client as any).authorizationError;
        client.destroy();
        resolve(JSON.stringify({
          server: JSON.parse(data),
          // deno-lint-ignore no-explicit-any
          clientAuthorized: (client as any).authorized,
          clientAuthorizationError: ce?.code ?? ce,
        }));
      });
      client.on("error", reject);
    });

    const result = JSON.parse(await promise);
    assertEquals(result.server.authorized, false);
    assertEquals(
      result.server.authorizationError,
      "DEPTH_ZERO_SELF_SIGNED_CERT",
    );
    assertEquals(result.clientAuthorized, false);
    assertEquals(
      result.clientAuthorizationError,
      "DEPTH_ZERO_SELF_SIGNED_CERT",
    );
    server.close();
    await new Promise<void>((resolve) => server.on("close", resolve));
  },
);

Deno.test("tls.getCACertificates returns bundled certificates", () => {
  const certs = tls.getCACertificates("bundled");
  assert(Array.isArray(certs));
  assert(certs.length > 0);
  assert(certs.every((cert) => typeof cert === "string"));
  assert(certs.every((cert) => cert.startsWith("-----BEGIN CERTIFICATE-----")));
});

Deno.test("tls.getCACertificates defaults to 'default'", () => {
  const certs = tls.getCACertificates();
  assert(Array.isArray(certs));
  assert(certs.length > 0);
});

Deno.test("tls.getCACertificates 'system' returns array", () => {
  const certs = tls.getCACertificates("system");
  assert(Array.isArray(certs));
  assert(certs.every((cert) => typeof cert === "string"));
});

Deno.test("tls.getCACertificates 'extra' returns empty array without NODE_EXTRA_CA_CERTS", () => {
  const certs = tls.getCACertificates("extra");
  assert(Array.isArray(certs));
  assertEquals(certs.length, 0);
});

Deno.test("tls.getCACertificates throws on invalid type", () => {
  assertThrows(
    () => {
      // deno-lint-ignore no-explicit-any
      (tls as any).getCACertificates("invalid");
    },
    TypeError,
  );
});

Deno.test("tls.setDefaultCACertificates exists", () => {
  // deno-lint-ignore no-explicit-any
  assertEquals(typeof (tls as any).setDefaultCACertificates, "function");
});

Deno.test("tls.setDefaultCACertificates validates input - must be array", () => {
  assertThrows(
    () => {
      // deno-lint-ignore no-explicit-any
      (tls as any).setDefaultCACertificates("not an array");
    },
    TypeError,
    "must be an instance of Array",
  );
});

Deno.test("tls.setDefaultCACertificates validates input - array elements must be strings or ArrayBufferView", () => {
  assertThrows(
    () => {
      // deno-lint-ignore no-explicit-any
      (tls as any).setDefaultCACertificates([123, 456]);
    },
    TypeError,
    "must be of type string or an instance of ArrayBufferView",
  );
});

Deno.test("tls.setDefaultCACertificates accepts valid certificate array", () => {
  // deno-lint-ignore no-explicit-any
  (tls as any).setDefaultCACertificates([rootCaCert]);
});

Deno.test("tls default options ignore runtime NODE_OPTIONS and execArgv mutations", async () => {
  const [status, output] = await execCode(`
    process.env.NODE_OPTIONS = "--tls-min-v1.0 --use-system-ca";
    process.execArgv.push("--tls-min-v1.0", "--use-system-ca");
    const tls = await import("node:tls");
    console.log(tls.DEFAULT_MIN_VERSION);
  `);
  assertEquals(status, 0);
  assertEquals(output.trim(), "TLSv1.2");
});

// https://github.com/denoland/deno/issues/31759
// Server-side STARTTLS: new tls.TLSSocket(socket, { isServer: true }) must
// auto-start the TLS handshake without requiring an explicit _start() call.
// This is used by SMTP, IMAP, XMPP, and similar STARTTLS protocols.
Deno.test("tls.TLSSocket server-side STARTTLS auto-starts handshake", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();

  const server = net.createServer((rawSocket) => {
    rawSocket.write("READY");
    rawSocket.once("data", (data) => {
      if (data.toString() === "STARTTLS") {
        rawSocket.write("OK", () => {
          // Server-side STARTTLS: no explicit _start() call
          const tlsSocket = new tls.TLSSocket(rawSocket, {
            isServer: true,
            key,
            cert,
            // deno-lint-ignore no-explicit-any
          } as any);
          tlsSocket.on("secure", () => {
            tlsSocket.write("SECURE");
          });
          tlsSocket.on("error", () => {});
        });
      }
    });
  });

  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    const socket = net.connect({ host: "localhost", port });
    socket.once("data", (greeting) => {
      assertEquals(greeting.toString(), "READY");
      socket.write("STARTTLS");
      socket.once("data", (response) => {
        assertEquals(response.toString(), "OK");
        const tlsSocket = tls.connect({
          socket,
          host: "localhost",
          ca: rootCaCert,
        });
        tlsSocket.on("secureConnect", () => {
          assert(tlsSocket.authorized);
        });
        tlsSocket.setEncoding("utf8");
        tlsSocket.on("data", (d) => {
          assertEquals(d, "SECURE");
          tlsSocket.destroy();
          server.close();
          resolve();
        });
        tlsSocket.on("error", (err: Error) => {
          server.close();
          reject(err);
        });
      });
    });
  });

  await promise;
});

// https://github.com/denoland/deno/issues/33296
// Regression test: tls.connect({ socket, host }) must send SNI derived from host.
// pg (PostgreSQL client) does STARTTLS: exchanges plaintext over TCP then calls
// tls.connect({ socket, host }) to upgrade. Without SNI, SNI-dependent servers
// (e.g. Neon PostgreSQL) drop the connection.
Deno.test("tls.connect socket upgrade sends SNI from host option", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();

  // Server that checks SNI was received
  const server = net.createServer((rawSocket) => {
    rawSocket.once("data", (data) => {
      if (data.toString() === "STARTTLS") {
        rawSocket.write("OK", () => {
          const tlsSocket = new tls.TLSSocket(rawSocket, {
            isServer: true,
            key,
            cert,
            // deno-lint-ignore no-explicit-any
          } as any);
          // deno-lint-ignore no-explicit-any
          (tlsSocket as any)._start();
          tlsSocket.on("secure", () => {
            tlsSocket.write("hello");
          });
          tlsSocket.on("error", () => {});
        });
      }
    });
  });

  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    const socket = net.connect({ host: "localhost", port });
    socket.on("connect", () => {
      // Exchange plaintext first (like pg SSLRequest/S)
      socket.write("STARTTLS");
      socket.once("data", (data) => {
        assertEquals(data.toString(), "OK");
        // Upgrade to TLS with host but no explicit servername
        const tlsSocket = tls.connect({
          socket,
          host: "localhost",
          ca: rootCaCert,
        });
        tlsSocket.on("secureConnect", () => {
          assert(tlsSocket.authorized, "Connection should be authorized");
          tlsSocket.destroy();
          server.close();
          resolve();
        });
        tlsSocket.on("error", (err: Error) => {
          server.close();
          reject(err);
        });
      });
    });
  });

  await promise;
});

// https://github.com/denoland/deno/issues/33296
// Regression test: tls.connect({ socket }) without host should derive SNI
// from the underlying socket's _host property.
Deno.test("tls.connect socket upgrade derives SNI from socket._host", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();

  const server = net.createServer((rawSocket) => {
    rawSocket.once("data", (data) => {
      if (data.toString() === "STARTTLS") {
        rawSocket.write("OK", () => {
          const tlsSocket = new tls.TLSSocket(rawSocket, {
            isServer: true,
            key,
            cert,
            // deno-lint-ignore no-explicit-any
          } as any);
          // deno-lint-ignore no-explicit-any
          (tlsSocket as any)._start();
          tlsSocket.on("secure", () => {
            tlsSocket.write("hello");
          });
          tlsSocket.on("error", () => {});
        });
      }
    });
  });

  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    // Connect with host="localhost" so socket._host is set
    const socket = net.connect({ host: "localhost", port });
    socket.on("connect", () => {
      socket.write("STARTTLS");
      socket.once("data", (data) => {
        assertEquals(data.toString(), "OK");
        // Upgrade without host or servername - should use socket._host
        const tlsSocket = tls.connect({
          socket,
          // No host or servername!
          ca: rootCaCert,
          rejectUnauthorized: false,
        });
        tlsSocket.on("secureConnect", () => {
          tlsSocket.destroy();
          server.close();
          resolve();
        });
        tlsSocket.on("error", (err: Error) => {
          server.close();
          reject(err);
        });
      });
    });
  });

  await promise;
});

// https://github.com/denoland/deno/issues/30170
// https://github.com/denoland/deno/issues/33391
// TLS server without cert/key should emit tlsClientError, not crash with
// an uncaught exception on stdout.  With a real TLS client the cert
// resolver is asked for a certificate, returns None, and rustls aborts
// the handshake with a fatal alert — surfaced as "no suitable signature
// algorithm" server-side.
Deno.test("tls server without certs emits tlsClientError instead of crashing", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<Error>();

  // Server with no cert/key — the cert resolver always returns None.
  const server = tls.createServer((_socket) => {
    reject(new Error("should not reach request handler"));
  });

  server.on("tlsClientError", (err: Error) => {
    resolve(err);
  });

  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    const client = tls.connect({
      port,
      host: "127.0.0.1",
      rejectUnauthorized: false,
    });
    client.on("error", () => {});
  });

  const err = await promise;
  assertMatch(err.message, /no suitable signature algorithm/i);

  server.close();
  await new Promise<void>((r) => server.on("close", r));
});

Deno.test("tls.connect strips trailing dot from servername", async () => {
  const listener = Deno.listenTls({
    port: 0,
    key,
    cert,
  });

  const conn = tls.connect({
    host: "localhost",
    port: listener.addr.port,
    // Use trailing dot - should be normalized to "localhost"
    servername: "localhost.",
    secureContext: {
      ca: rootCaCert,
      // deno-lint-ignore no-explicit-any
    } as any,
  });

  const serverConn = await listener.accept();

  const { promise: connected, resolve: resolveConnected } = Promise
    .withResolvers<void>();
  conn.on("secureConnect", () => {
    assert(conn.authorized, "Connection should be authorized");
    resolveConnected();
  });

  conn.on("error", (err: Error) => {
    // Should not get a certificate error with trailing dot
    throw err;
  });

  await connected;
  conn.destroy();
  serverConn.close();
  listener.close();
  await new Promise((resolve) => conn.on("close", resolve));
});

// https://github.com/denoland/deno/issues/33743
Deno.test("TLSSocket.setServername throws Node-compatible coded errors", () => {
  const clientSocket = new tls.TLSSocket(new net.Socket());
  const typeErr = assertThrows(
    // @ts-expect-error testing invalid input
    () => clientSocket.setServername(123),
    TypeError,
  );
  assertEquals(
    (typeErr as TypeError & { code?: string }).code,
    "ERR_INVALID_ARG_TYPE",
  );
  clientSocket.destroy();

  const serverSocket = new tls.TLSSocket(new net.Socket(), { isServer: true });
  const sniErr = assertThrows(
    // @ts-ignore setServername is missing from the bundled @types/node
    () => serverSocket.setServername("example.com"),
    Error,
    "Cannot issue SNI from a TLS server-side socket",
  );
  assertEquals(
    (sniErr as Error & { code?: string }).code,
    "ERR_TLS_SNI_FROM_SERVER",
  );
  serverSocket.destroy();
});

// Regression: tls.createSecureContext must accept the documented array forms
// of `cert`, `key` and `pfx`. An empty `pfx: []` (as produced by playwright's
// APIRequestContext) used to throw "not enough data", and array forms of
// cert/key were silently coerced via String() into unusable values.
Deno.test("[node/tls] createSecureContext accepts array cert/key/pfx", () => {
  // Empty pfx array is a no-op (regression test for #34371).
  const ctx1 = tls.createSecureContext({ pfx: [] });
  assert(ctx1);

  // Cert as Buffer[] is concatenated into a single PEM string
  // (regression test for #34367).
  const ctx2 = tls.createSecureContext({
    cert: [cert],
    key: [{ pem: key }],
  });
  assertStringIncludes(ctx2.context.cert as string, "BEGIN CERTIFICATE");
  assertStringIncludes(ctx2.context.key as string, "PRIVATE KEY");

  // Multiple PEM blocks via array stay parseable: both certs are present.
  const ctx3 = tls.createSecureContext({ cert: [cert, cert] });
  const certBlocks =
    (ctx3.context.cert as string).match(/BEGIN CERTIFICATE/g) ?? [];
  assertEquals(certBlocks.length, 2);

  // A malformed pfx still throws.
  assertThrows(
    () => tls.createSecureContext({ pfx: "short" }),
    Error,
    "not enough data",
  );
  assertThrows(
    () => tls.createSecureContext({ pfx: ["short"] }),
    Error,
    "not enough data",
  );
});

// https://github.com/denoland/deno/issues/34336
// Default OpenSSL 3 PFX bundles use a SHA-256 MAC, and Node accepts them.
// Older bundles can use SHA-1, SHA-384, or SHA-512.
for (const alg of ["sha1", "sha256", "sha384", "sha512"] as const) {
  Deno.test(`tls.createSecureContext accepts pfx with ${alg} MAC`, () => {
    const pfx = Buffer.from(
      Deno.readFileSync(join(tlsTestdataDir, `localhost_${alg}.pfx`)),
    );
    const ctx = tls.createSecureContext({ pfx, passphrase: "secret" });
    assert(ctx);
  });
}

Deno.test("tls.createSecureContext rejects pfx with wrong passphrase", () => {
  const pfx = Buffer.from(
    Deno.readFileSync(join(tlsTestdataDir, "localhost_sha256.pfx")),
  );
  assertThrows(
    () => tls.createSecureContext({ pfx, passphrase: "wrong" }),
    Error,
    "mac verify failure",
  );
});

// https://github.com/denoland/deno/issues/34434
// `openssl pkcs12 -export` without -legacy emits PBES2 + PBKDF2 + AES-256-CBC
// for both the cert bag and the shrouded key bag. This is the default shape
// on OpenSSL 3.x and the one Node interoperates with; -legacy (SHA-1/RC2-40)
// is the only shape the old code path accepted, and Node rejects that one.
Deno.test("tls.createSecureContext accepts modern pfx (PBES2/AES-256-CBC)", () => {
  const pfx = Buffer.from(
    Deno.readFileSync(join(tlsTestdataDir, "localhost_modern.pfx")),
  );
  const ctx = tls.createSecureContext({ pfx, passphrase: "secret" });
  assert(ctx);
});

// A modern (MAC'd) PFX with the wrong passphrase fails at MAC verification,
// before any bag is decrypted, so the error matches the legacy fixtures.
Deno.test("tls.createSecureContext rejects modern pfx with wrong passphrase", () => {
  const pfx = Buffer.from(
    Deno.readFileSync(join(tlsTestdataDir, "localhost_modern.pfx")),
  );
  assertThrows(
    () => tls.createSecureContext({ pfx, passphrase: "wrong" }),
    Error,
    "mac verify failure",
  );
});

// A PFX produced without a MAC (`openssl pkcs12 -export -nomac`) is still
// accepted, matching Node/OpenSSL which treat the MAC as optional. The certs
// are stored in plaintext and only the key is shrouded, so a wrong passphrase
// gets past the (absent) MAC and surfaces as a key-decrypt failure rather than
// "mac verify failure"; this exercises the PBES2 shrouded-key path directly.
Deno.test("tls.createSecureContext accepts modern pfx without a MAC", () => {
  const pfx = Buffer.from(
    Deno.readFileSync(join(tlsTestdataDir, "localhost_modern_nomac.pfx")),
  );
  const ctx = tls.createSecureContext({ pfx, passphrase: "secret" });
  assert(ctx);
});

Deno.test("tls.createSecureContext reports key decrypt failure on bad passphrase", () => {
  const pfx = Buffer.from(
    Deno.readFileSync(join(tlsTestdataDir, "localhost_modern_nomac.pfx")),
  );
  assertThrows(
    () => tls.createSecureContext({ pfx, passphrase: "wrong" }),
    Error,
    "failed to decrypt PFX private key",
  );
});

// A PFX bundling a chain (`-certfile RootCA.pem`) carries more than one cert
// bag. The first bag is taken as the leaf and the rest become the CA chain,
// so `ca` must hold exactly the RootCA cert and the leaf must not leak into
// it. This also exercises decrypting an EncryptedData envelope that holds
// multiple cert bags.
Deno.test("tls.createSecureContext extracts the CA chain from a pfx", () => {
  const pfx = Buffer.from(
    Deno.readFileSync(join(tlsTestdataDir, "localhost_modern_chain.pfx")),
  );
  const ctx = tls.createSecureContext({ pfx, passphrase: "secret" });
  // deno-lint-ignore no-explicit-any
  const context = (ctx as any).context;
  assert(typeof context.cert === "string" && context.cert.length > 0);
  assert(globalThis.Array.isArray(context.ca));
  assertEquals(context.ca.length, 1);
  // The chained CA cert landed in `ca`, distinct from the leaf cert.
  assert(context.ca[0].includes("BEGIN CERTIFICATE"));
  assert(context.ca[0] !== context.cert);
});
