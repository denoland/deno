// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
  assertThrowsAsync,
  deferred,
} from "./test_util.ts";
import { BufReader, BufWriter } from "../../../test_util/std/io/bufio.ts";
import { TextProtoReader } from "../../../test_util/std/textproto/mod.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

Deno.test("connectTLSInvalidHost", async function (): Promise<void> {
  const listener = await Deno.listenTls({
    hostname: "localhost",
    port: 3567,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key",
  });

  await assertThrowsAsync(async () => {
    await Deno.connectTls({ hostname: "127.0.0.1", port: 3567 });
  }, Error);

  listener.close();
});

Deno.test("listenTLSNonExistentCertKeyFiles", function (): void {
  const options = {
    hostname: "localhost",
    port: 3500,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key",
  };

  assertThrows(() => {
    Deno.listenTls({
      ...options,
      certFile: "./non/existent/file",
    });
  }, Deno.errors.NotFound);

  assertThrows(() => {
    Deno.listenTls({
      ...options,
      keyFile: "./non/existent/file",
    });
  }, Deno.errors.NotFound);
});

Deno.test("listenTLSEmptyKeyFile", function (): void {
  const options = {
    hostname: "localhost",
    port: 3500,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key",
  };

  const testDir = Deno.makeTempDirSync();
  const keyFilename = testDir + "/key.pem";
  Deno.writeFileSync(keyFilename, new Uint8Array([]), {
    mode: 0o666,
  });

  assertThrows(() => {
    Deno.listenTls({
      ...options,
      keyFile: keyFilename,
    });
  }, Error);
});

Deno.test("listenTLSEmptyCertFile", function (): void {
  const options = {
    hostname: "localhost",
    port: 3500,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key",
  };

  const testDir = Deno.makeTempDirSync();
  const certFilename = testDir + "/cert.crt";
  Deno.writeFileSync(certFilename, new Uint8Array([]), {
    mode: 0o666,
  });

  assertThrows(() => {
    Deno.listenTls({
      ...options,
      certFile: certFilename,
    });
  }, Error);
});

Deno.test("dialAndListenTLS", async function (): Promise<void> {
  const resolvable = deferred();
  const hostname = "localhost";
  const port = 3500;

  const listener = Deno.listenTls({
    hostname,
    port,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key",
  });

  const response = encoder.encode(
    "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
  );

  listener.accept().then(
    async (conn): Promise<void> => {
      assert(conn.remoteAddr != null);
      assert(conn.localAddr != null);
      await conn.write(response);
      // TODO(bartlomieju): this might be a bug
      setTimeout(() => {
        conn.close();
        resolvable.resolve();
      }, 0);
    },
  );

  const conn = await Deno.connectTls({
    hostname,
    port,
    certFile: "cli/tests/tls/RootCA.pem",
  });
  assert(conn.rid > 0);
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
  const headers = await tpr.readMIMEHeader();
  assert(headers !== null);
  const contentLength = parseInt(headers.get("content-length")!);
  const bodyBuf = new Uint8Array(contentLength);
  await r.readFull(bodyBuf);
  assertEquals(decoder.decode(bodyBuf), "Hello World\n");
  conn.close();
  listener.close();
  await resolvable;
});

async function tlsPair(port: number): Promise<[Deno.Conn, Deno.Conn]> {
  const listener = Deno.listenTls({
    hostname: "localhost",
    port,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key",
  });

  const acceptPromise = listener.accept();
  const connectPromise = Deno.connectTls({
    hostname: "localhost",
    port,
    certFile: "cli/tests/tls/RootCA.pem",
  });
  const connections = await Promise.all([acceptPromise, connectPromise]);

  listener.close();

  return connections;
}

async function sendCloseWrite(conn: Deno.Conn): Promise<void> {
  const buf = new Uint8Array(1024);
  let n: number | null;

  // Send 1.
  n = await conn.write(new Uint8Array([1]));
  assertStrictEquals(n, 1);

  // Send EOF.
  await conn.closeWrite();

  // Receive 2.
  n = await conn.read(buf);
  assertStrictEquals(n, 1);
  assertStrictEquals(buf[0], 2);

  conn.close();
}

async function receiveCloseWrite(conn: Deno.Conn): Promise<void> {
  const buf = new Uint8Array(1024);
  let n: number | null;

  // Receive 1.
  n = await conn.read(buf);
  assertStrictEquals(n, 1);
  assertStrictEquals(buf[0], 1);

  // Receive EOF.
  n = await conn.read(buf);
  assertStrictEquals(n, null);

  // Send 2.
  n = await conn.write(new Uint8Array([2]));
  assertStrictEquals(n, 1);

  conn.close();
}

async function sendAlotReceiveNothing(conn: Deno.Conn): Promise<void> {
  // Start receive op.
  const readBuf = new Uint8Array(1024);
  const readPromise = conn.read(readBuf);

  // Send 1 MB of data.
  const writeBuf = new Uint8Array(1 << 20);
  writeBuf.fill(42);
  await conn.write(writeBuf);

  // Send EOF.
  await conn.closeWrite();

  // Close the connection.
  conn.close();

  // Read op should be canceled.
  await assertThrowsAsync(
    async () => await readPromise,
    Deno.errors.Interrupted,
  );
}

async function receiveAlotSendNothing(conn: Deno.Conn): Promise<void> {
  const readBuf = new Uint8Array(1024);
  let n: number | null;

  // Receive 1 MB of data.
  for (let nread = 0; nread < 1 << 20; nread += n!) {
    n = await conn.read(readBuf);
    assertStrictEquals(typeof n, "number");
    assert(n! > 0);
    assertStrictEquals(readBuf[0], 42);
  }

  // Close the connection, without sending anything at all.
  conn.close();
}

Deno.test("tlsServerStreamHalfClose", async function (): Promise<void> {
  const [serverConn, clientConn] = await tlsPair(3501);
  await Promise.all([
    sendCloseWrite(serverConn),
    receiveCloseWrite(clientConn),
  ]);
});

Deno.test("tlsClientStreamHalfClose", async function (): Promise<void> {
  const [serverConn, clientConn] = await tlsPair(3502);
  await Promise.all([
    sendCloseWrite(clientConn),
    receiveCloseWrite(serverConn),
  ]);
});

Deno.test("tlsServerStreamCancelRead", async function (): Promise<void> {
  const [serverConn, clientConn] = await tlsPair(3503);
  await Promise.all([
    sendAlotReceiveNothing(serverConn),
    receiveAlotSendNothing(clientConn),
  ]);
});

Deno.test("tlsClientStreamCancelRead", async function (): Promise<void> {
  const [serverConn, clientConn] = await tlsPair(3504);
  await Promise.all([
    sendAlotReceiveNothing(clientConn),
    receiveAlotSendNothing(serverConn),
  ]);
});

Deno.test("startTls", async function (): Promise<void> {
  const hostname = "smtp.gmail.com";
  const port = 587;
  const encoder = new TextEncoder();

  let conn = await Deno.connect({
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

  conn = await Deno.startTls(conn, { hostname });
  writer = new BufWriter(conn);
  reader = new TextProtoReader(new BufReader(conn));

  // After that use TLS communication again
  await writer.write(encoder.encode(`EHLO ${hostname}\r\n`));
  await writer.flush();

  while ((line = (await reader.readLine()) as string)) {
    assert(line.startsWith("250"));
    if (line.startsWith("250 ")) break;
  }

  conn.close();
});

Deno.test("connectTLSCertFileNoReadPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.connectTls({
      hostname: "github.com",
      port: 443,
      certFile: "cli/tests/tls/RootCA.crt",
    });
  }, Deno.errors.PermissionDenied);
});

Deno.test("connectTLSNoPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "net" });

  await assertThrowsAsync(async () => {
    await Deno.connectTls({ hostname: "github.com", port: 443 });
  }, Deno.errors.PermissionDenied);
});

Deno.test("listenTLSNoReadPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.listenTls({
      hostname: "localhost",
      port: 3500,
      certFile: "cli/tests/tls/localhost.crt",
      keyFile: "cli/tests/tls/localhost.key",
    });
  }, Deno.errors.PermissionDenied);
});
