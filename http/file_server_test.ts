// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { readFile, run } = Deno;

import { test } from "../testing/mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";

let fileServer;
async function startFileServer(): Promise<void> {
  fileServer = run({
    args: [
      "deno",
      "--allow-read",
      "--allow-net",
      "http/file_server.ts",
      ".",
      "--cors"
    ],
    stdout: "piped"
  });
  // Once fileServer is ready it will write to its stdout.
  const r = new TextProtoReader(new BufReader(fileServer.stdout));
  const [s, err] = await r.readLine();
  assert(err == null);
  assert(s.includes("server listening"));
}
function killFileServer(): void {
  fileServer.close();
  fileServer.stdout.close();
}

test(async function serveFile(): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4500/azure-pipelines.yml");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.headers.get("content-type"), "text/yaml; charset=utf-8");
    const downloadedFile = await res.text();
    const localFile = new TextDecoder().decode(
      await readFile("./azure-pipelines.yml")
    );
    assertEquals(downloadedFile, localFile);
  } finally {
    killFileServer();
  }
});

test(async function serveDirectory(): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4500/");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    const page = await res.text();
    assert(page.includes("azure-pipelines.yml"));
  } finally {
    killFileServer();
  }
});

test(async function serveFallback(): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4500/badfile.txt");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEquals(res.status, 404);
  } finally {
    killFileServer();
  }
});
