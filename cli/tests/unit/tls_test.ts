// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  deferred,
  unitTest,
} from "./test_util.ts";
import { BufReader, BufWriter } from "../../../std/io/bufio.ts";
import { TextProtoReader } from "../../../std/textproto/mod.ts";
import { resolve } from "../../../std/path/win32.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

unitTest(async function connectTLSNoPerm(): Promise<void> {
  await assertThrowsAsync(async () => {
    await Deno.connectTls({ hostname: "github.com", port: 443 });
  }, Deno.errors.PermissionDenied);
});

unitTest(async function connectTLSCertFileNoReadPerm(): Promise<void> {
  await assertThrowsAsync(async () => {
    await Deno.connectTls({
      hostname: "github.com",
      port: 443,
      certFile: "cli/tests/tls/RootCA.crt",
    });
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { perms: { read: true, net: true } },
  function listenTLSNonExistentCertKeyFiles(): void {
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
  },
);

unitTest({ perms: { net: true } }, function listenTLSNoReadPerm(): void {
  assertThrows(() => {
    Deno.listenTls({
      hostname: "localhost",
      port: 3500,
      certFile: "cli/tests/tls/localhost.crt",
      keyFile: "cli/tests/tls/localhost.key",
    });
  }, Deno.errors.PermissionDenied);
});

unitTest(
  {
    perms: { read: true, write: true, net: true },
  },
  function listenTLSEmptyKeyFile(): void {
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
  },
);

unitTest(
  { perms: { read: true, write: true, net: true } },
  function listenTLSEmptyCertFile(): void {
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
  },
);

unitTest(
  { perms: { read: true, net: true } },
  async function dialAndListenTLS(): Promise<void> {
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
  },
);

unitTest(
  { perms: { read: true, net: true } },
  async function startTls(): Promise<void> {
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
  },
);
unitTest(
  { perms: { net: true, read: true } },
  async function getAndConnectWithSNI(): Promise<void> {
    const resolvable = deferred();
    const hostname = "localhost";
    const port = 3600;
    const certFile = "cli/tests/tls/localhost.crt";
    const keyFile = "cli/tests/tls/localhost.key";
    const rootCertFile = "cli/tests/tls/RootCa.pem";

    const listener = Deno.listenTls({
      hostname,
      port,
      certFile,
      keyFile,
    });

    listener.accept().then((conn) => {
      assertEquals((conn as Deno.Conn & { sni?: string }).sni, hostname);
      setTimeout(() => {
        listener.close();
        conn.close();
        resolvable.resolve();
      }, 0);
    });

    async function connect() {
      const conn = await Deno.connectTls({
        hostname,
        port,
        certFile: rootCertFile,
      });
      conn.close();
    }

    await resolvable;
  },
);
