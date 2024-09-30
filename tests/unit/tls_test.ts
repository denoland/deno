// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
  assertStrictEquals,
  assertThrows,
} from "./test_util.ts";
import { BufReader, BufWriter } from "@std/io";
import { readAll } from "@std/io/read-all";
import { writeAll } from "@std/io/write-all";
import { TextProtoReader } from "../testdata/run/textproto.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
const caCerts = [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")];

async function sleep(msec: number) {
  await new Promise((res, _rej) => setTimeout(res, msec));
}

function listenTls(
  options?: { alpnProtocols?: string[]; reusePort?: boolean },
): { listener: Deno.TlsListener; port: number; hostname: string } {
  const tlsOptions = { port: 0, hostname: "localhost", cert, key, ...options };
  const listener = Deno.listenTls(tlsOptions);
  return {
    listener,
    port: (<Deno.NetAddr> listener.addr).port,
    hostname: "localhost",
  };
}

function listenTcp(): {
  listener: Deno.Listener;
  port: number;
  hostname: string;
} {
  const listener = Deno.listen({ port: 0, hostname: "localhost" });
  return {
    listener,
    port: (<Deno.NetAddr> listener.addr).port,
    hostname: "localhost",
  };
}

function unreachable(): never {
  throw new Error("Unreachable code reached");
}

Deno.test({ permissions: { net: false } }, async function connectTLSNoPerm() {
  await assertRejects(async () => {
    await Deno.connectTls({ hostname: "deno.land", port: 443 });
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { read: true, net: true } },
  async function connectTLSInvalidHost() {
    await assertRejects(async () => {
      await Deno.connectTls({ hostname: "256.0.0.0", port: 3567 });
    }, TypeError);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function startTlsWithoutExclusiveAccessToTcpConn() {
    const { listener, hostname, port } = listenTcp();
    const [serverConn, clientConn] = await Promise.all([
      listener.accept(),
      Deno.connect({ hostname, port }),
    ]);

    const buf = new Uint8Array(128);
    const readPromise = clientConn.read(buf);
    // `clientConn` is being used by a pending promise (`readPromise`) so
    // `Deno.startTls` cannot consume the connection.
    await assertRejects(
      () => Deno.startTls(clientConn, { hostname }),
      Deno.errors.Busy,
    );

    serverConn.close();
    listener.close();
    await readPromise;
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function dialAndListenTLS() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const { listener, port, hostname } = listenTls();

    const response = encoder.encode(
      "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
    );

    listener.accept().then(
      async (conn) => {
        assert(conn.remoteAddr != null);
        assert(conn.localAddr != null);
        await conn.write(response);
        // TODO(bartlomieju): this might be a bug
        setTimeout(() => {
          conn.close();
          resolve();
        }, 0);
      },
    );

    const conn = await Deno.connectTls({ hostname, port, caCerts });
    const w = new BufWriter(conn);
    const r = new BufReader(conn);
    const body = `GET / HTTP/1.1\r\nHost: ${hostname}:${port}\r\n\r\n`;
    const writeResult = await w.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await w.flush();
    const tpr = new TextProtoReader(r);
    const statusLine = await tpr.readLine();
    assert(statusLine !== null, `line must be read: ${String(statusLine)}`);
    const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
    assert(m !== null, "must be matched");
    const [_, proto, status, ok] = m;
    assertEquals(proto, "HTTP/1.1");
    assertEquals(status, "200");
    assertEquals(ok, "OK");
    const headers = await tpr.readMimeHeader();
    assert(headers !== null);
    const contentLength = parseInt(headers.get("content-length")!);
    const bodyBuf = new Uint8Array(contentLength);
    await r.readFull(bodyBuf);
    assertEquals(decoder.decode(bodyBuf), "Hello World\n");
    conn.close();
    listener.close();
    await promise;
  },
);

Deno.test(
  { permissions: { read: false, net: true } },
  async function listenTlsWithCertAndKey() {
    const { promise, resolve } = Promise.withResolvers<void>();

    const { listener, hostname, port } = listenTls();

    const response = encoder.encode(
      "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
    );

    listener.accept().then(
      async (conn) => {
        assert(conn.remoteAddr != null);
        assert(conn.localAddr != null);
        await conn.write(response);
        setTimeout(() => {
          conn.close();
          resolve();
        }, 0);
      },
    );

    const conn = await Deno.connectTls({ hostname, port, caCerts });
    const w = new BufWriter(conn);
    const r = new BufReader(conn);
    const body = `GET / HTTP/1.1\r\nHost: ${hostname}:${port}\r\n\r\n`;
    const writeResult = await w.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await w.flush();
    const tpr = new TextProtoReader(r);
    const statusLine = await tpr.readLine();
    assert(statusLine !== null, `line must be read: ${String(statusLine)}`);
    const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
    assert(m !== null, "must be matched");
    const [_, proto, status, ok] = m;
    assertEquals(proto, "HTTP/1.1");
    assertEquals(status, "200");
    assertEquals(ok, "OK");
    const headers = await tpr.readMimeHeader();
    assert(headers !== null);
    const contentLength = parseInt(headers.get("content-length")!);
    const bodyBuf = new Uint8Array(contentLength);
    await r.readFull(bodyBuf);
    assertEquals(decoder.decode(bodyBuf), "Hello World\n");
    conn.close();
    listener.close();
    await promise;
  },
);

async function tlsPair(): Promise<[Deno.Conn, Deno.Conn]> {
  const { listener, hostname, port } = listenTls();

  const acceptPromise = listener.accept();
  const connectPromise = Deno.connectTls({
    hostname,
    port,
    caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
  });
  const endpoints = await Promise.all([acceptPromise, connectPromise]);

  listener.close();

  return endpoints;
}

async function tlsAlpn(
  useStartTls: boolean,
): Promise<[Deno.TlsConn, Deno.TlsConn]> {
  const { listener, port } = listenTls({
    alpnProtocols: ["deno", "rocks"],
  });

  const acceptPromise = listener.accept();

  const caCerts = [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")];
  const clientAlpnProtocols = ["rocks", "rises"];
  let endpoints: [Deno.TlsConn, Deno.TlsConn];

  if (!useStartTls) {
    const connectPromise = Deno.connectTls({
      hostname: "localhost",
      port,
      caCerts,
      alpnProtocols: clientAlpnProtocols,
    });
    endpoints = await Promise.all([acceptPromise, connectPromise]);
  } else {
    const client = await Deno.connect({
      hostname: "localhost",
      port,
    });
    const connectPromise = Deno.startTls(client, {
      hostname: "localhost",
      caCerts,
      alpnProtocols: clientAlpnProtocols,
    });
    endpoints = await Promise.all([acceptPromise, connectPromise]);
  }

  listener.close();
  return endpoints;
}

async function sendThenCloseWriteThenReceive(
  conn: Deno.Conn,
  chunkCount: number,
  chunkSize: number,
) {
  const byteCount = chunkCount * chunkSize;
  const buf = new Uint8Array(chunkSize); // Note: buf is size of _chunk_.
  let n: number;

  // Slowly send 42s.
  buf.fill(42);
  for (let remaining = byteCount; remaining > 0; remaining -= n) {
    n = await conn.write(buf.subarray(0, remaining));
    assert(n >= 1);
    await sleep(10);
  }

  // Send EOF.
  await conn.closeWrite();

  // Receive 69s.
  for (let remaining = byteCount; remaining > 0; remaining -= n) {
    buf.fill(0);
    n = await conn.read(buf) as number;
    assert(n >= 1);
    assertStrictEquals(buf[0], 69);
    assertStrictEquals(buf[n - 1], 69);
  }

  conn.close();
}

async function receiveThenSend(
  conn: Deno.Conn,
  chunkCount: number,
  chunkSize: number,
) {
  const byteCount = chunkCount * chunkSize;
  const buf = new Uint8Array(byteCount); // Note: buf size equals `byteCount`.
  let n: number;

  // Receive 42s.
  for (let remaining = byteCount; remaining > 0; remaining -= n) {
    buf.fill(0);
    n = await conn.read(buf) as number;
    assert(n >= 1);
    assertStrictEquals(buf[0], 42);
    assertStrictEquals(buf[n - 1], 42);
  }

  // Slowly send 69s.
  buf.fill(69);
  for (let remaining = byteCount; remaining > 0; remaining -= n) {
    n = await conn.write(buf.subarray(0, remaining));
    assert(n >= 1);
    await sleep(10);
  }

  conn.close();
}

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerAlpnListenConnect() {
    const [serverConn, clientConn] = await tlsAlpn(false);
    const [serverHS, clientHS] = await Promise.all([
      serverConn.handshake(),
      clientConn.handshake(),
    ]);
    assertStrictEquals(serverHS.alpnProtocol, "rocks");
    assertStrictEquals(clientHS.alpnProtocol, "rocks");

    serverConn.close();
    clientConn.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerAlpnListenStartTls() {
    const [serverConn, clientConn] = await tlsAlpn(true);
    const [serverHS, clientHS] = await Promise.all([
      serverConn.handshake(),
      clientConn.handshake(),
    ]);
    assertStrictEquals(serverHS.alpnProtocol, "rocks");
    assertStrictEquals(clientHS.alpnProtocol, "rocks");

    serverConn.close();
    clientConn.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerStreamHalfCloseSendOneByte() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(serverConn, 1, 1),
      receiveThenSend(clientConn, 1, 1),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientStreamHalfCloseSendOneByte() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(clientConn, 1, 1),
      receiveThenSend(serverConn, 1, 1),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerStreamHalfCloseSendOneChunk() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(serverConn, 1, 1 << 20 /* 1 MB */),
      receiveThenSend(clientConn, 1, 1 << 20 /* 1 MB */),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientStreamHalfCloseSendOneChunk() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(clientConn, 1, 1 << 20 /* 1 MB */),
      receiveThenSend(serverConn, 1, 1 << 20 /* 1 MB */),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerStreamHalfCloseSendManyBytes() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(serverConn, 100, 1),
      receiveThenSend(clientConn, 100, 1),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientStreamHalfCloseSendManyBytes() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(clientConn, 100, 1),
      receiveThenSend(serverConn, 100, 1),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerStreamHalfCloseSendManyChunks() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(serverConn, 100, 1 << 16 /* 64 kB */),
      receiveThenSend(clientConn, 100, 1 << 16 /* 64 kB */),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientStreamHalfCloseSendManyChunks() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendThenCloseWriteThenReceive(clientConn, 100, 1 << 16 /* 64 kB */),
      receiveThenSend(serverConn, 100, 1 << 16 /* 64 kB */),
    ]);
  },
);

const largeAmount = 1 << 20 /* 1 MB */;

async function sendAlotReceiveNothing(conn: Deno.Conn) {
  // Start receive op.
  const readBuf = new Uint8Array(1024);
  const readPromise = conn.read(readBuf);

  const timeout = setTimeout(() => {
    throw new Error("Failed to send buffer in a reasonable amount of time");
  }, 10_000);

  // Send 1 MB of data.
  const writeBuf = new Uint8Array(largeAmount);
  writeBuf.fill(42);
  await writeAll(conn, writeBuf);

  clearTimeout(timeout);

  // Send EOF.
  await conn.closeWrite();

  // Close the connection.
  conn.close();

  // Read op should be canceled.
  await assertRejects(
    async () => await readPromise,
    Deno.errors.Interrupted,
  );
}

async function receiveAlotSendNothing(conn: Deno.Conn) {
  const readBuf = new Uint8Array(1024);
  let n: number | null;
  let nread = 0;

  const timeout = setTimeout(() => {
    throw new Error(
      `Failed to read buffer in a reasonable amount of time (got ${nread}/${largeAmount})`,
    );
  }, 10_000);

  // Receive 1 MB of data.
  try {
    for (; nread < largeAmount; nread += n!) {
      n = await conn.read(readBuf);
      assertStrictEquals(typeof n, "number");
      assert(n! > 0);
      assertStrictEquals(readBuf[0], 42);
    }
  } catch (e) {
    throw new Error(
      `Got an error (${
        (e as Error).message
      }) after reading ${nread}/${largeAmount} bytes`,
      { cause: e },
    );
  }
  clearTimeout(timeout);

  // Close the connection, without sending anything at all.
  conn.close();
}

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerStreamCancelRead() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendAlotReceiveNothing(serverConn),
      receiveAlotSendNothing(clientConn),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientStreamCancelRead() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendAlotReceiveNothing(clientConn),
      receiveAlotSendNothing(serverConn),
    ]);
  },
);

async function sendReceiveEmptyBuf(conn: Deno.Conn) {
  const byteBuf = new Uint8Array([1]);
  const emptyBuf = new Uint8Array(0);
  let n: number | null;

  n = await conn.write(emptyBuf);
  assertStrictEquals(n, 0);

  n = await conn.read(emptyBuf);
  assertStrictEquals(n, 0);

  n = await conn.write(byteBuf);
  assertStrictEquals(n, 1);

  n = await conn.read(byteBuf);
  assertStrictEquals(n, 1);

  await conn.closeWrite();

  n = await conn.write(emptyBuf);
  assertStrictEquals(n, 0);

  await assertRejects(async () => {
    await conn.write(byteBuf);
  }, Deno.errors.NotConnected);

  n = await conn.write(emptyBuf);
  assertStrictEquals(n, 0);

  n = await conn.read(byteBuf);
  assertStrictEquals(n, null);

  conn.close();
}

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsStreamSendReceiveEmptyBuf() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      sendReceiveEmptyBuf(serverConn),
      sendReceiveEmptyBuf(clientConn),
    ]);
  },
);

function immediateClose(conn: Deno.Conn) {
  conn.close();
  return Promise.resolve();
}

async function closeWriteAndClose(conn: Deno.Conn) {
  await conn.closeWrite();

  if (await conn.read(new Uint8Array(1)) !== null) {
    throw new Error("did not expect to receive data on TLS stream");
  }

  conn.close();
}

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsServerStreamImmediateClose() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      immediateClose(serverConn),
      closeWriteAndClose(clientConn),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientStreamImmediateClose() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      closeWriteAndClose(serverConn),
      immediateClose(clientConn),
    ]);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsClientAndServerStreamImmediateClose() {
    const [serverConn, clientConn] = await tlsPair();
    await Promise.all([
      immediateClose(serverConn),
      immediateClose(clientConn),
    ]);
  },
);

async function tlsWithTcpFailureTestImpl(
  phase: "handshake" | "traffic",
  cipherByteCount: number,
  failureMode: "corruption" | "shutdown",
  reverse: boolean,
) {
  const tls = listenTls();
  const tcp = listenTcp();

  const [tlsServerConn, tcpServerConn] = await Promise.all([
    tls.listener.accept(),
    Deno.connect({ hostname: tls.hostname, port: tls.port }),
  ]);

  const [tcpClientConn, tlsClientConn] = await Promise.all([
    tcp.listener.accept(),
    Deno.connectTls({
      hostname: tcp.hostname,
      port: tcp.port,
      caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
    }),
  ]);

  tls.listener.close();
  tcp.listener.close();

  const {
    tlsConn1,
    tlsConn2,
    tcpConn1,
    tcpConn2,
  } = reverse
    ? {
      tlsConn1: tlsClientConn,
      tlsConn2: tlsServerConn,
      tcpConn1: tcpClientConn,
      tcpConn2: tcpServerConn,
    }
    : {
      tlsConn1: tlsServerConn,
      tlsConn2: tlsClientConn,
      tcpConn1: tcpServerConn,
      tcpConn2: tcpClientConn,
    };

  const tcpForwardingInterruptDeferred1 = Promise.withResolvers<void>();
  const tcpForwardingPromise1 = forwardBytes(
    tcpConn2,
    tcpConn1,
    cipherByteCount,
    tcpForwardingInterruptDeferred1,
  );

  const tcpForwardingInterruptDeferred2 = Promise.withResolvers<void>();
  const tcpForwardingPromise2 = forwardBytes(
    tcpConn1,
    tcpConn2,
    Infinity,
    tcpForwardingInterruptDeferred2,
  );

  switch (phase) {
    case "handshake": {
      let expectedError;
      switch (failureMode) {
        case "corruption":
          expectedError = Deno.errors.InvalidData;
          break;
        case "shutdown":
          expectedError = Deno.errors.UnexpectedEof;
          break;
        default:
          unreachable();
      }

      const tlsTrafficPromise1 = Promise.all([
        assertRejects(
          () => sendBytes(tlsConn1, 0x01, 1),
          expectedError,
        ),
        assertRejects(
          () => receiveBytes(tlsConn1, 0x02, 1),
          expectedError,
        ),
      ]);

      const tlsTrafficPromise2 = Promise.all([
        assertRejects(
          () => sendBytes(tlsConn2, 0x02, 1),
          Deno.errors.UnexpectedEof,
        ),
        assertRejects(
          () => receiveBytes(tlsConn2, 0x01, 1),
          Deno.errors.UnexpectedEof,
        ),
      ]);

      await tcpForwardingPromise1;

      switch (failureMode) {
        case "corruption":
          await sendBytes(tcpConn1, 0xff, 1 << 14 /* 16 kB */);
          break;
        case "shutdown":
          await tcpConn1.closeWrite();
          break;
        default:
          unreachable();
      }
      await tlsTrafficPromise1;

      tcpForwardingInterruptDeferred2.resolve();
      await tcpForwardingPromise2;
      await tcpConn2.closeWrite();
      await tlsTrafficPromise2;

      break;
    }

    case "traffic": {
      await Promise.all([
        sendBytes(tlsConn2, 0x88, 8888),
        receiveBytes(tlsConn1, 0x88, 8888),
        sendBytes(tlsConn1, 0x99, 99999),
        receiveBytes(tlsConn2, 0x99, 99999),
      ]);

      tcpForwardingInterruptDeferred1.resolve();
      await tcpForwardingInterruptDeferred1.promise;

      switch (failureMode) {
        case "corruption":
          await sendBytes(tcpConn1, 0xff, 1 << 14 /* 16 kB */);
          await assertRejects(
            () => receiveEof(tlsConn1),
            Deno.errors.InvalidData,
          );
          tcpForwardingInterruptDeferred2.resolve();
          break;
        case "shutdown":
          await Promise.all([
            tcpConn1.closeWrite(),
            await assertRejects(
              () => receiveEof(tlsConn1),
              Deno.errors.UnexpectedEof,
            ),
            await tlsConn1.closeWrite(),
            await receiveEof(tlsConn2),
          ]);
          break;
        default:
          unreachable();
      }

      await tcpForwardingPromise2;

      break;
    }

    default:
      unreachable();
  }

  tlsServerConn.close();
  tlsClientConn.close();
  tcpServerConn.close();
  tcpClientConn.close();

  async function sendBytes(
    conn: Deno.Conn,
    byte: number,
    count: number,
  ) {
    let buf = new Uint8Array(1 << 12 /* 4 kB */);
    buf.fill(byte);

    while (count > 0) {
      buf = buf.subarray(0, Math.min(buf.length, count));
      const nwritten = await conn.write(buf);
      assertStrictEquals(nwritten, buf.length);
      count -= nwritten;
    }
  }

  async function receiveBytes(
    conn: Deno.Conn,
    byte: number,
    count: number,
  ) {
    let buf = new Uint8Array(1 << 12 /* 4 kB */);
    while (count > 0) {
      buf = buf.subarray(0, Math.min(buf.length, count));
      const r = await conn.read(buf);
      assertNotEquals(r, null);
      assert(buf.subarray(0, r!).every((b) => b === byte));
      count -= r!;
    }
  }

  async function receiveEof(conn: Deno.Conn) {
    const buf = new Uint8Array(1);
    const r = await conn.read(buf);
    assertStrictEquals(r, null);
  }

  async function forwardBytes(
    source: Deno.Conn,
    sink: Deno.Conn,
    count: number,
    interruptPromise: ReturnType<typeof Promise.withResolvers<void>>,
  ) {
    let buf = new Uint8Array(1 << 12 /* 4 kB */);
    while (count > 0) {
      buf = buf.subarray(0, Math.min(buf.length, count));
      const nread = await Promise.race([
        source.read(buf),
        interruptPromise.promise,
      ]);
      if (nread == null) break; // Either EOF or interrupted.
      const nwritten = await sink.write(buf.subarray(0, nread));
      assertStrictEquals(nread, nwritten);
      count -= nwritten;
    }
  }
}

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeWithTcpCorruptionImmediately() {
    await tlsWithTcpFailureTestImpl("handshake", 0, "corruption", false);
    await tlsWithTcpFailureTestImpl("handshake", 0, "corruption", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeWithTcpShutdownImmediately() {
    await tlsWithTcpFailureTestImpl("handshake", 0, "shutdown", false);
    await tlsWithTcpFailureTestImpl("handshake", 0, "shutdown", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeWithTcpCorruptionAfter70Bytes() {
    await tlsWithTcpFailureTestImpl("handshake", 76, "corruption", false);
    await tlsWithTcpFailureTestImpl("handshake", 78, "corruption", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeWithTcpShutdownAfter70bytes() {
    await tlsWithTcpFailureTestImpl("handshake", 77, "shutdown", false);
    await tlsWithTcpFailureTestImpl("handshake", 79, "shutdown", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeWithTcpCorruptionAfter200Bytes() {
    await tlsWithTcpFailureTestImpl("handshake", 200, "corruption", false);
    await tlsWithTcpFailureTestImpl("handshake", 202, "corruption", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeWithTcpShutdownAfter200bytes() {
    await tlsWithTcpFailureTestImpl("handshake", 201, "shutdown", false);
    await tlsWithTcpFailureTestImpl("handshake", 203, "shutdown", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsTrafficWithTcpCorruption() {
    await tlsWithTcpFailureTestImpl("traffic", Infinity, "corruption", false);
    await tlsWithTcpFailureTestImpl("traffic", Infinity, "corruption", true);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsTrafficWithTcpShutdown() {
    await tlsWithTcpFailureTestImpl("traffic", Infinity, "shutdown", false);
    await tlsWithTcpFailureTestImpl("traffic", Infinity, "shutdown", true);
  },
);

function createHttpsListener(): {
  listener: Deno.TlsListener;
  hostname: string;
  port: number;
} {
  // Query format: `curl --insecure https://localhost:8443/z/12345`
  // The server returns a response consisting of 12345 times the letter 'z'.
  const { listener, hostname, port } = listenTls();

  serve(listener);
  return { listener, hostname, port };

  async function serve(listener: Deno.Listener) {
    for await (const conn of listener) {
      const EOL = "\r\n";

      // Read GET request plus headers.
      const buf = new Uint8Array(1 << 12 /* 4 kB */);
      const decoder = new TextDecoder();
      let req = "";
      while (!req.endsWith(EOL + EOL)) {
        const n = await conn.read(buf);
        if (n === null) throw new Error("Unexpected EOF");
        req += decoder.decode(buf.subarray(0, n));
      }

      // Parse GET request.
      const { filler, count, version } =
        /^GET \/(?<filler>[^\/]+)\/(?<count>\d+) HTTP\/(?<version>1\.\d)\r\n/
          .exec(req)!.groups as {
            filler: string;
            count: string;
            version: string;
          };

      // Generate response.
      const resBody = new TextEncoder().encode(filler.repeat(+count));
      const resHead = new TextEncoder().encode(
        [
          `HTTP/${version} 200 OK`,
          `Content-Length: ${resBody.length}`,
          "Content-Type: text/plain",
        ].join(EOL) + EOL + EOL,
      );

      // Send response.
      await writeAll(conn, resHead);
      await writeAll(conn, resBody);

      // Close TCP connection.
      conn.close();
    }
  }
}

async function curl(url: string): Promise<string> {
  const { success, code, stdout, stderr } = await new Deno.Command("curl", {
    args: ["--insecure", url],
  }).output();

  if (!success) {
    throw new Error(
      `curl ${url} failed: ${code}:\n${new TextDecoder().decode(stderr)}`,
    );
  }
  return new TextDecoder().decode(stdout);
}

Deno.test(
  { permissions: { read: true, net: true, run: true } },
  async function curlFakeHttpsServer() {
    const { listener, port } = createHttpsListener();

    const res1 = await curl(`https://localhost:${port}/d/1`);
    assertStrictEquals(res1, "d");

    const res2 = await curl(`https://localhost:${port}/e/12345`);
    assertStrictEquals(res2, "e".repeat(12345));

    const count3 = 1 << 17; // 128 kB.
    const res3 = await curl(`https://localhost:${port}/n/${count3}`);
    assertStrictEquals(res3, "n".repeat(count3));

    const count4 = 12345678;
    const res4 = await curl(`https://localhost:${port}/o/${count4}`);
    assertStrictEquals(res4, "o".repeat(count4));

    listener.close();
  },
);

Deno.test(
  // Ignored because gmail appears to reject us on CI sometimes
  { ignore: true, permissions: { read: true, net: true } },
  async function startTls() {
    const hostname = "smtp.gmail.com";
    const port = 587;
    const encoder = new TextEncoder();

    const conn = await Deno.connect({
      hostname,
      port,
    });

    let writer = new BufWriter(conn);
    let reader = new TextProtoReader(new BufReader(conn));

    let line: string | null = (await reader.readLine()) as string;
    assert(line.startsWith("220"));

    await writer.write(encoder.encode(`EHLO ${hostname}\r\n`));
    await writer.flush();

    while ((line = (await reader.readLine()) as string)) {
      assert(line.startsWith("250"));
      if (line.startsWith("250 ")) break;
    }

    await writer.write(encoder.encode("STARTTLS\r\n"));
    await writer.flush();

    line = await reader.readLine();

    // Received the message that the server is ready to establish TLS
    assertEquals(line, "220 2.0.0 Ready to start TLS");

    const tlsConn = await Deno.startTls(conn, { hostname });
    writer = new BufWriter(tlsConn);
    reader = new TextProtoReader(new BufReader(tlsConn));

    // After that use TLS communication again
    await writer.write(encoder.encode(`EHLO ${hostname}\r\n`));
    await writer.flush();

    while ((line = (await reader.readLine()) as string)) {
      assert(line.startsWith("250"));
      if (line.startsWith("250 ")) break;
    }

    tlsConn.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function connectTLSBadCertKey(): Promise<void> {
    await assertRejects(async () => {
      await Deno.connectTls({
        hostname: "deno.land",
        port: 443,
        cert: "bad data",
        key: Deno.readTextFileSync(
          "tests/testdata/tls/localhost.key",
        ),
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function connectTLSBadKey(): Promise<void> {
    await assertRejects(async () => {
      await Deno.connectTls({
        hostname: "deno.land",
        port: 443,
        cert: Deno.readTextFileSync(
          "tests/testdata/tls/localhost.crt",
        ),
        key: "bad data",
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function connectTLSNotKey(): Promise<void> {
    await assertRejects(async () => {
      await Deno.connectTls({
        hostname: "deno.land",
        port: 443,
        cert: Deno.readTextFileSync(
          "tests/testdata/tls/localhost.crt",
        ),
        key: "",
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function connectWithCert() {
    // The test_server running on port 4552 responds with 'PASS' if client
    // authentication was successful. Try it by running test_server and
    //   curl --key cli/tests/testdata/tls/localhost.key \
    //        --cert cli/tests/testdata/tls/localhost.crt \
    //        --cacert cli/tests/testdata/tls/RootCA.crt https://localhost:4552/
    const conn = await Deno.connectTls({
      hostname: "localhost",
      port: 4552,
      cert: Deno.readTextFileSync(
        "tests/testdata/tls/localhost.crt",
      ),
      key: Deno.readTextFileSync(
        "tests/testdata/tls/localhost.key",
      ),
      caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
    });
    const result = decoder.decode(await readAll(conn));
    assertEquals(result, "PASS");
    conn.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function connectTLSCaCerts() {
    const conn = await Deno.connectTls({
      hostname: "localhost",
      port: 4557,
      caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
    });
    const result = decoder.decode(await readAll(conn));
    assertEquals(result, "PASS");
    conn.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function startTLSCaCerts() {
    const plainConn = await Deno.connect({
      hostname: "localhost",
      port: 4557,
    });
    const conn = await Deno.startTls(plainConn, {
      hostname: "localhost",
      caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
    });
    const result = decoder.decode(await readAll(conn));
    assertEquals(result, "PASS");
    conn.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeSuccess() {
    const { listener, hostname, port } = listenTls();

    const acceptPromise = listener.accept();
    const connectPromise = Deno.connectTls({
      hostname,
      port,
      caCerts: [await Deno.readTextFile("tests/testdata/tls/RootCA.crt")],
    });
    const [conn1, conn2] = await Promise.all([acceptPromise, connectPromise]);
    listener.close();

    await Promise.all([conn1.handshake(), conn2.handshake()]);

    // Begin sending a 10mb blob over the TLS connection.
    const whole = new Uint8Array(10 << 20); // 10mb.
    whole.fill(42);
    const sendPromise = writeAll(conn1, whole);
    // Set up the other end to receive half of the large blob.
    const half = new Uint8Array(whole.byteLength / 2);
    const receivePromise = readFull(conn2, half);

    await conn1.handshake();
    await conn2.handshake();

    // Finish receiving the first 5mb.
    assertEquals(await receivePromise, half.length);

    // See that we can call `handshake()` in the middle of large reads and writes.
    await conn1.handshake();
    await conn2.handshake();

    // Receive second half of large blob. Wait for the send promise and check it.
    assertEquals(await readFull(conn2, half), half.length);
    await sendPromise;

    await conn1.handshake();
    await conn2.handshake();

    await conn1.closeWrite();
    await conn2.closeWrite();

    await conn1.handshake();
    await conn2.handshake();

    conn1.close();
    conn2.close();

    async function readFull(conn: Deno.Conn, buf: Uint8Array) {
      let offset, n;
      for (offset = 0; offset < buf.length; offset += n) {
        n = await conn.read(buf.subarray(offset, buf.length));
        assert(n != null && n > 0);
      }
      return offset;
    }
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function tlsHandshakeFailure() {
    let tls: { listener: Deno.TlsListener; port: number; hostname: string };

    async function server() {
      for await (const conn of tls.listener) {
        for (let i = 0; i < 10; i++) {
          // Handshake fails because the client rejects the server certificate.
          await assertRejects(
            () => conn.handshake(),
            Deno.errors.InvalidData,
            "received fatal alert",
          );
        }
        conn.close();
        break;
      }
    }

    async function connectTlsClient() {
      const conn = await Deno.connectTls({
        hostname: tls.hostname,
        port: tls.port,
      });
      // Handshake fails because the server presents a self-signed certificate.
      await assertRejects(
        () => conn.handshake(),
        Deno.errors.InvalidData,
        "invalid peer certificate: UnknownIssuer",
      );
      conn.close();
    }

    tls = listenTls();
    await Promise.all([server(), connectTlsClient()]);

    async function startTlsClient() {
      const tcpConn = await Deno.connect({
        hostname: tls.hostname,
        port: tls.port,
      });
      const tlsConn = await Deno.startTls(tcpConn, {
        hostname: "foo.land",
        caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
      });
      // Handshake fails because hostname doesn't match the certificate.
      await assertRejects(
        () => tlsConn.handshake(),
        Deno.errors.InvalidData,
        "NotValidForName",
      );
      tlsConn.close();
    }

    tls = listenTls();
    await Promise.all([server(), startTlsClient()]);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function listenTlsWithReuseAddr() {
    const deferred1 = Promise.withResolvers<void>();
    const { listener: listener1, port, hostname } = listenTls();

    listener1.accept().then((conn) => {
      conn.close();
      deferred1.resolve();
    });

    const conn1 = await Deno.connectTls({ hostname, port, caCerts });
    conn1.close();
    await deferred1.promise;
    listener1.close();

    const deferred2 = Promise.withResolvers<void>();
    const listener2 = Deno.listenTls({ hostname, port, cert, key });

    listener2.accept().then((conn) => {
      conn.close();
      deferred2.resolve();
    });

    const conn2 = await Deno.connectTls({ hostname, port, caCerts });
    conn2.close();
    await deferred2.promise;
    listener2.close();
  },
);

Deno.test({
  ignore: Deno.build.os !== "linux",
  permissions: { net: true },
}, async function listenTlsReusePort() {
  const { listener: listener1, port, hostname } = listenTls({
    reusePort: true,
  });
  const listener2 = Deno.listenTls({
    hostname,
    port,
    cert,
    key,
    reusePort: true,
  });
  let p1;
  let p2;
  let listener1Recv = false;
  let listener2Recv = false;
  while (!listener1Recv || !listener2Recv) {
    if (!p1) {
      p1 = listener1.accept().then((conn) => {
        conn.close();
        listener1Recv = true;
        p1 = undefined;
        listener1.close();
      }).catch(() => {});
    }
    if (!p2) {
      p2 = listener2.accept().then((conn) => {
        conn.close();
        listener2Recv = true;
        p2 = undefined;
        listener2.close();
      }).catch(() => {});
    }
    const conn = await Deno.connectTls({ hostname, port, caCerts });
    conn.close();
    await Promise.race([p1, p2]);
  }
});

Deno.test({
  ignore: Deno.build.os === "linux",
  permissions: { net: true },
}, function listenTlsReusePortDoesNothing() {
  const { listener: listener1, hostname, port } = listenTls({
    reusePort: true,
  });
  assertThrows(() => {
    Deno.listenTls({ hostname, port, cert, key, reusePort: true });
  }, Deno.errors.AddrInUse);
  listener1.close();
});

Deno.test({
  permissions: { net: true },
}, function listenTlsDoesNotThrowOnStringPort() {
  const listener = Deno.listenTls({
    hostname: "localhost",
    // @ts-ignore String port is not allowed by typing, but it shouldn't throw
    // for backwards compatibility.
    port: "0",
    cert,
    key,
  });
  listener.close();
});

Deno.test(
  { permissions: { net: true, read: true } },
  function listenTLSInvalidCert() {
    assertThrows(() => {
      Deno.listenTls({
        hostname: "localhost",
        port: 0,
        cert: Deno.readTextFileSync("tests/testdata/tls/invalid.crt"),
        key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  function listenTLSInvalidKey() {
    assertThrows(() => {
      Deno.listenTls({
        hostname: "localhost",
        port: 0,
        cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
        key: Deno.readTextFileSync("tests/testdata/tls/invalid.key"),
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  function listenTLSEcKey() {
    const listener = Deno.listenTls({
      hostname: "localhost",
      port: 0,
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost_ecc.crt"),
      key: Deno.readTextFileSync("tests/testdata/tls/localhost_ecc.key"),
    });
    listener.close();
  },
);
