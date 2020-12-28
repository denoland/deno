// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../testing/asserts.ts";
import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { ServerRequest } from "./server.ts";
import { FileServerArgs, serveFile } from "./file_server.ts";
import { dirname, fromFileUrl, join, resolve } from "../path/mod.ts";
let fileServer: Deno.Process<Deno.RunOptions & { stdout: "piped" }>;

type FileServerCfg = Omit<FileServerArgs, "_"> & { target?: string };

const moduleDir = dirname(fromFileUrl(import.meta.url));
const testdataDir = resolve(moduleDir, "testdata");

async function startFileServer({
  target = ".",
  port = 4507,
  "dir-listing": dirListing = true,
}: FileServerCfg = {}): Promise<void> {
  fileServer = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "--allow-net",
      "file_server.ts",
      target,
      "--cors",
      "-p",
      `${port}`,
      `${dirListing ? "" : "--no-dir-listing"}`,
    ],
    cwd: moduleDir,
    stdout: "piped",
    stderr: "null",
  });
  // Once fileServer is ready it will write to its stdout.
  assert(fileServer.stdout != null);
  const r = new TextProtoReader(new BufReader(fileServer.stdout));
  const s = await r.readLine();
  assert(s !== null && s.includes("server listening"));
}

async function startFileServerAsLibrary({}: FileServerCfg = {}): Promise<void> {
  fileServer = await Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "--allow-net",
      "testdata/file_server_as_library.ts",
    ],
    cwd: moduleDir,
    stdout: "piped",
    stderr: "null",
  });
  assert(fileServer.stdout != null);
  const r = new TextProtoReader(new BufReader(fileServer.stdout));
  const s = await r.readLine();
  assert(s !== null && s.includes("Server running..."));
}

async function killFileServer(): Promise<void> {
  fileServer.close();
  // Process.close() kills the file server process. However this termination
  // happens asynchronously, and since we've just closed the process resource,
  // we can't use `await fileServer.status()` to wait for the process to have
  // exited. As a workaround, wait for its stdout to close instead.
  // TODO(piscisaureus): when `Process.kill()` is stable and works on Windows,
  // switch to calling `kill()` followed by `await fileServer.status()`.
  await Deno.readAll(fileServer.stdout!);
  fileServer.stdout!.close();
}

Deno.test(
  "file_server serveFile",
  async (): Promise<void> => {
    await startFileServer();
    try {
      const res = await fetch("http://localhost:4507/README.md");
      assert(res.headers.has("access-control-allow-origin"));
      assert(res.headers.has("access-control-allow-headers"));
      assertEquals(res.headers.get("content-type"), "text/markdown");
      const downloadedFile = await res.text();
      const localFile = new TextDecoder().decode(
        await Deno.readFile(join(moduleDir, "README.md")),
      );
      assertEquals(downloadedFile, localFile);
    } finally {
      await killFileServer();
    }
  },
);

Deno.test(
  "file_server serveFile in testdata",
  async (): Promise<void> => {
    await startFileServer({ target: "./testdata" });
    try {
      const res = await fetch("http://localhost:4507/hello.html");
      assert(res.headers.has("access-control-allow-origin"));
      assert(res.headers.has("access-control-allow-headers"));
      assertEquals(res.headers.get("content-type"), "text/html");
      const downloadedFile = await res.text();
      const localFile = new TextDecoder().decode(
        await Deno.readFile(join(testdataDir, "hello.html")),
      );
      assertEquals(downloadedFile, localFile);
    } finally {
      await killFileServer();
    }
  },
);

Deno.test("serveDirectory", async function (): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4507/");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    const page = await res.text();
    assert(page.includes("README.md"));

    // `Deno.FileInfo` is not completely compatible with Windows yet
    // TODO: `mode` should work correctly in the future.
    // Correct this test case accordingly.
    Deno.build.os !== "windows" &&
      assert(/<td class="mode">(\s)*\([a-zA-Z-]{10}\)(\s)*<\/td>/.test(page));
    Deno.build.os === "windows" &&
      assert(/<td class="mode">(\s)*\(unknown mode\)(\s)*<\/td>/.test(page));
    assert(page.includes(`<a href="/README.md">README.md</a>`));
  } finally {
    await killFileServer();
  }
});

Deno.test("serveFallback", async function (): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4507/badfile.txt");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 404);
    const _ = await res.text();
  } finally {
    await killFileServer();
  }
});

Deno.test("checkPathTraversal", async function (): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch(
      "http://localhost:4507/../../../../../../../..",
    );
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 200);
    const listing = await res.text();
    assertStringIncludes(listing, "README.md");
  } finally {
    await killFileServer();
  }
});

Deno.test("checkURIEncodedPathTraversal", async function (): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch(
      "http://localhost:4507/%2F..%2F..%2F..%2F..%2F..%2F..%2F..%2F..",
    );
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 404);
    const _ = await res.text();
  } finally {
    await killFileServer();
  }
});

Deno.test("serveWithUnorthodoxFilename", async function (): Promise<void> {
  await startFileServer();
  try {
    let res = await fetch("http://localhost:4507/testdata/%");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 200);
    let _ = await res.text();
    res = await fetch("http://localhost:4507/testdata/test%20file.txt");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 200);
    _ = await res.text();
  } finally {
    await killFileServer();
  }
});

Deno.test("printHelp", async function (): Promise<void> {
  const helpProcess = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      // TODO(ry) It ought to be possible to get the help output without
      // --allow-read.
      "--allow-read",
      "file_server.ts",
      "--help",
    ],
    cwd: moduleDir,
    stdout: "piped",
  });
  assert(helpProcess.stdout != null);
  const r = new TextProtoReader(new BufReader(helpProcess.stdout));
  const s = await r.readLine();
  assert(s !== null && s.includes("Deno File Server"));
  helpProcess.close();
  helpProcess.stdout.close();
});

Deno.test("contentType", async () => {
  const request = new ServerRequest();
  const response = await serveFile(request, join(testdataDir, "hello.html"));
  const contentType = response.headers!.get("content-type");
  assertEquals(contentType, "text/html");
  (response.body as Deno.File).close();
});

Deno.test("file_server running as library", async function (): Promise<void> {
  await startFileServerAsLibrary();
  try {
    const res = await fetch("http://localhost:8000");
    assertEquals(res.status, 200);
    const _ = await res.text();
  } finally {
    await killFileServer();
  }
});

Deno.test("file_server should ignore query params", async () => {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4507/README.md?key=value");
    assertEquals(res.status, 200);
    const downloadedFile = await res.text();
    const localFile = new TextDecoder().decode(
      await Deno.readFile(join(moduleDir, "README.md")),
    );
    assertEquals(downloadedFile, localFile);
  } finally {
    await killFileServer();
  }
});

async function startTlsFileServer({
  target = ".",
  port = 4577,
}: FileServerCfg = {}): Promise<void> {
  fileServer = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "--allow-net",
      "file_server.ts",
      target,
      "--host",
      "localhost",
      "--cert",
      "./testdata/tls/localhost.crt",
      "--key",
      "./testdata/tls/localhost.key",
      "--cors",
      "-p",
      `${port}`,
    ],
    cwd: moduleDir,
    stdout: "piped",
    stderr: "null",
  });
  // Once fileServer is ready it will write to its stdout.
  assert(fileServer.stdout != null);
  const r = new TextProtoReader(new BufReader(fileServer.stdout));
  const s = await r.readLine();
  assert(s !== null && s.includes("server listening"));
}

Deno.test("serveDirectory TLS", async function (): Promise<void> {
  await startTlsFileServer();
  try {
    // Valid request after invalid
    const conn = await Deno.connectTls({
      hostname: "localhost",
      port: 4577,
      certFile: join(testdataDir, "tls/RootCA.pem"),
    });

    await Deno.writeAll(
      conn,
      new TextEncoder().encode("GET / HTTP/1.0\r\n\r\n"),
    );
    const res = new Uint8Array(128 * 1024);
    const nread = await conn.read(res);
    assert(nread !== null);
    conn.close();
    const page = new TextDecoder().decode(res.subarray(0, nread));
    assert(page.includes("<title>Deno File Server</title>"));
  } finally {
    await killFileServer();
  }
});

Deno.test("partial TLS arguments fail", async function (): Promise<void> {
  fileServer = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "--allow-net",
      "file_server.ts",
      ".",
      "--host",
      "localhost",
      "--cert",
      "./testdata/tls/localhost.crt",
      "-p",
      `4578`,
    ],
    cwd: moduleDir,
    stdout: "piped",
    stderr: "null",
  });
  try {
    // Once fileServer is ready it will write to its stdout.
    assert(fileServer.stdout != null);
    const r = new TextProtoReader(new BufReader(fileServer.stdout));
    const s = await r.readLine();
    assert(
      s !== null && s.includes("--key and --cert are required for TLS"),
    );
  } finally {
    await killFileServer();
  }
});

Deno.test("file_server disable dir listings", async function (): Promise<void> {
  await startFileServer({ "dir-listing": false });
  try {
    const res = await fetch("http://localhost:4507/");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 404);
    const _ = await res.text();
  } finally {
    await killFileServer();
  }
});
