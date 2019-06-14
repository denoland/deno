// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { readFile, run, stat, makeTempDir, remove, env } = Deno;

import { test, runIfMain, TestFunction } from "../testing/mod.ts";
import { assert, assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
import { BufReader, EOF } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { install, uninstall } from "./mod.ts";
import * as path from "../fs/path.ts";

let fileServer: Deno.Process;

// copied from `http/file_server_test.ts`
async function startFileServer(): Promise<void> {
  fileServer = run({
    args: [
      "deno",
      "run",
      "--allow-read",
      "--allow-net",
      "http/file_server.ts",
      ".",
      "--cors"
    ],
    stdout: "piped"
  });
  // Once fileServer is ready it will write to its stdout.
  const r = new TextProtoReader(new BufReader(fileServer.stdout!));
  const s = await r.readLine();
  assert(s !== EOF && s.includes("server listening"));
}

function killFileServer(): void {
  fileServer.close();
  fileServer.stdout!.close();
}

function installerTest(t: TestFunction): void {
  const fn = async (): Promise<void> => {
    await startFileServer();
    const tempDir = await makeTempDir();
    const envVars = env();
    const originalHomeDir = envVars["HOME"];
    envVars["HOME"] = tempDir;

    try {
      await t();
    } finally {
      killFileServer();
      await remove(tempDir, { recursive: true });
      envVars["HOME"] = originalHomeDir;
    }
  };

  test(fn);
}

installerTest(async function installBasic(): Promise<void> {
  await install("file_srv", "http://localhost:4500/http/file_server.ts", []);

  const { HOME } = env();
  const filePath = path.resolve(HOME, ".deno/bin/file_srv");
  const fileInfo = await stat(filePath);
  assert(fileInfo.isFile());

  const fileBytes = await readFile(filePath);
  const fileContents = new TextDecoder().decode(fileBytes);
  assertEquals(
    fileContents,
    "#/bin/sh\ndeno http://localhost:4500/http/file_server.ts $@"
  );
});

installerTest(async function installWithFlags(): Promise<void> {
  await install("file_server", "http://localhost:4500/http/file_server.ts", [
    "--allow-net",
    "--allow-read",
    "--foobar"
  ]);

  const { HOME } = env();
  const filePath = path.resolve(HOME, ".deno/bin/file_server");

  const fileBytes = await readFile(filePath);
  const fileContents = new TextDecoder().decode(fileBytes);
  assertEquals(
    fileContents,
    "#/bin/sh\ndeno --allow-net --allow-read http://localhost:4500/http/file_server.ts --foobar $@"
  );
});

installerTest(async function uninstallBasic(): Promise<void> {
  await install("file_server", "http://localhost:4500/http/file_server.ts", []);

  const { HOME } = env();
  const filePath = path.resolve(HOME, ".deno/bin/file_server");

  await uninstall("file_server");

  let thrown = false;
  try {
    await stat(filePath);
  } catch (e) {
    thrown = true;
    assert(e instanceof Deno.DenoError);
    assertEquals(e.kind, Deno.ErrorKind.NotFound);
  }

  assert(thrown);
});

installerTest(async function uninstallNonExistentModule(): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      await uninstall("file_server");
    },
    Error,
    "file_server not found"
  );
});

runIfMain(import.meta);
