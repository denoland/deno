// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../testing/asserts.ts";
import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { ServerRequest } from "./server.ts";
import { serveFile } from "./file_server.ts";
const { test } = Deno;
let fileServer: Deno.Process<Deno.RunOptions & { stdout: "piped" }>;

type FileServerCfg = {
  target?: string;
  port?: number;
};

async function startFileServer({
  target = ".",
  port = 4507,
}: FileServerCfg = {}): Promise<void> {
  fileServer = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "--allow-net",
      "http/file_server.ts",
      target,
      "--cors",
      "-p",
      `${port}`,
    ],
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
      "http/testdata/file_server_as_library.ts",
    ],
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

test("file_server serveFile in ./", async (): Promise<void> => {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4507/README.md");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.headers.get("content-type"), "text/markdown");
    const downloadedFile = await res.text();
    const localFile = new TextDecoder().decode(
      await Deno.readFile("README.md")
    );
    assertEquals(downloadedFile, localFile);
  } finally {
    await killFileServer();
  }
});

test("file_server serveFile in ./http", async (): Promise<void> => {
  await startFileServer({ target: "./http" });
  try {
    const res = await fetch("http://localhost:4507/README.md");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.headers.get("content-type"), "text/markdown");
    const downloadedFile = await res.text();
    const localFile = new TextDecoder().decode(
      await Deno.readFile("./http/README.md")
    );
    console.log(downloadedFile, localFile);
    assertEquals(downloadedFile, localFile);
  } finally {
    await killFileServer();
  }
});

test("serveDirectory", async function (): Promise<void> {
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

test("serveFallback", async function (): Promise<void> {
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

test("serveWithUnorthodoxFilename", async function (): Promise<void> {
  await startFileServer();
  try {
    let res = await fetch("http://localhost:4507/http/testdata/%");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 200);
    let _ = await res.text();
    res = await fetch("http://localhost:4507/http/testdata/test%20file.txt");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 200);
    _ = await res.text();
  } finally {
    await killFileServer();
  }
});

test("printHelp", async function (): Promise<void> {
  const helpProcess = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      // TODO(ry) It ought to be possible to get the help output without
      // --allow-read.
      "--allow-read",
      "http/file_server.ts",
      "--help",
    ],
    stdout: "piped",
  });
  assert(helpProcess.stdout != null);
  const r = new TextProtoReader(new BufReader(helpProcess.stdout));
  const s = await r.readLine();
  assert(s !== null && s.includes("Deno File Server"));
  helpProcess.close();
  helpProcess.stdout.close();
});

test("contentType", async () => {
  const request = new ServerRequest();
  const response = await serveFile(request, "http/testdata/hello.html");
  const contentType = response.headers!.get("content-type");
  assertEquals(contentType, "text/html");
  (response.body as Deno.File).close();
});

test("file_server running as library", async function (): Promise<void> {
  await startFileServerAsLibrary();
  try {
    const res = await fetch("http://localhost:8000");
    assertEquals(res.status, 200);
  } finally {
    await killFileServer();
  }
});
