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
    "must be an array",
  );
});

Deno.test("tls.setDefaultCACertificates validates input - array elements must be strings", () => {
  assertThrows(
    () => {
      // deno-lint-ignore no-explicit-any
      (tls as any).setDefaultCACertificates([123, 456]);
    },
    TypeError,
    "must be a string",
  );
});

Deno.test("tls.setDefaultCACertificates accepts valid certificate array", () => {
  const testCert = `-----BEGIN CERTIFICATE-----
MIIBkTCB+wIJAKHHCgVZU1FFMA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBnRl
c3RDQTAeFw0yMDAxMDEwMDAwMDBaFw0zMDAxMDEwMDAwMDBaMBExDzANBgNVBAMM
BnRlc3RDQTCB
-----END CERTIFICATE-----`;

  // deno-lint-ignore no-explicit-any
  (tls as any).setDefaultCACertificates([testCert]);
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
