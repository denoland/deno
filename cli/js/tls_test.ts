// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";
import { BufWriter, BufReader } from "../../std/io/bufio.ts";
import { TextProtoReader } from "../../std/textproto/mod.ts";
import { runIfMain } from "../../std/testing/mod.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

test(async function dialTLSNoPerm(): Promise<void> {
  let err;
  try {
    await Deno.dialTLS({ hostname: "github.com", port: 443 });
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

test(async function dialTLSCertFileNoReadPerm(): Promise<void> {
  let err;
  try {
    await Deno.dialTLS({
      hostname: "github.com",
      port: 443,
      certFile: "cli/tests/tls/RootCA.crt"
    });
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm(
  { read: true, net: true },
  async function listenTLSNonExistentCertKeyFiles(): Promise<void> {
    let err;
    const options = {
      hostname: "localhost",
      port: 4500,
      certFile: "cli/tests/tls/localhost.crt",
      keyFile: "cli/tests/tls/localhost.key"
    };

    try {
      Deno.listenTLS({
        ...options,
        certFile: "./non/existent/file"
      });
    } catch (e) {
      err = e;
    }
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");

    try {
      Deno.listenTLS({
        ...options,
        keyFile: "./non/existent/file"
      });
    } catch (e) {
      err = e;
    }
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");
  }
);

testPerm({ net: true }, async function listenTLSNoReadPerm(): Promise<void> {
  let err;
  try {
    Deno.listenTLS({
      hostname: "localhost",
      port: 4500,
      certFile: "cli/tests/tls/localhost.crt",
      keyFile: "cli/tests/tls/localhost.key"
    });
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm(
  { read: true, write: true, net: true },
  async function listenTLSEmptyKeyFile(): Promise<void> {
    let err;
    const options = {
      hostname: "localhost",
      port: 4500,
      certFile: "cli/tests/tls/localhost.crt",
      keyFile: "cli/tests/tls/localhost.key"
    };

    const testDir = Deno.makeTempDirSync();
    const keyFilename = testDir + "/key.pem";
    Deno.writeFileSync(keyFilename, new Uint8Array([]), {
      perm: 0o666
    });

    try {
      Deno.listenTLS({
        ...options,
        keyFile: keyFilename
      });
    } catch (e) {
      err = e;
    }
    assertEquals(err.kind, Deno.ErrorKind.Other);
    assertEquals(err.name, "Other");
  }
);

testPerm(
  { read: true, write: true, net: true },
  async function listenTLSEmptyCertFile(): Promise<void> {
    let err;
    const options = {
      hostname: "localhost",
      port: 4500,
      certFile: "cli/tests/tls/localhost.crt",
      keyFile: "cli/tests/tls/localhost.key"
    };

    const testDir = Deno.makeTempDirSync();
    const certFilename = testDir + "/cert.crt";
    Deno.writeFileSync(certFilename, new Uint8Array([]), {
      perm: 0o666
    });

    try {
      Deno.listenTLS({
        ...options,
        certFile: certFilename
      });
    } catch (e) {
      err = e;
    }
    assertEquals(err.kind, Deno.ErrorKind.Other);
    assertEquals(err.name, "Other");
  }
);

testPerm({ read: true, net: true }, async function dialAndListenTLS(): Promise<
  void
> {
  const hostname = "localhost";
  const port = 4500;

  const listener = Deno.listenTLS({
    hostname,
    port,
    certFile: "cli/tests/tls/localhost.crt",
    keyFile: "cli/tests/tls/localhost.key"
  });

  const response = encoder.encode(
    "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
  );

  listener.accept().then(
    async (conn): Promise<void> => {
      assert(conn.remoteAddr != null);
      assert(conn.localAddr != null);
      await conn.write(response);
      conn.close();
    }
  );

  const conn = await Deno.dialTLS({
    hostname,
    port,
    certFile: "cli/tests/tls/RootCA.pem"
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
  assert(!!statusLine, "line must be read: " + statusLine);
  const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
  assert(m !== null, "must be matched");
  const [_, proto, status, ok] = m;
  assertEquals(proto, "HTTP/1.1");
  assertEquals(status, "200");
  assertEquals(ok, "OK");
  const headers = await tpr.readMIMEHeader();
  const contentLength = parseInt(headers.get("content-length"));
  const bodyBuf = new Uint8Array(contentLength);
  await r.readFull(bodyBuf);
  assertEquals(decoder.decode(bodyBuf), "Hello World\n");
  conn.close();
});

runIfMain(import.meta);
