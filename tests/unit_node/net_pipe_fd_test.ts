// Copyright 2018-2026 the Deno authors. MIT license.
//
// Tests for Pipe.open(fd) via unix socket I/O.
// Verifies that FdStreamBase correctly handles read/write on raw fds.

import { assertEquals } from "@std/assert";
import { Buffer } from "node:buffer";
import * as net from "node:net";
import * as path from "node:path";
import * as os from "node:os";
import * as fs from "node:fs";

function tmpSockPath(): string {
  return path.join(
    os.tmpdir(),
    `deno_pipe_fd_test_${Deno.pid}_${Math.random().toString(36).slice(2)}.sock`,
  );
}

Deno.test({
  name: "unix socket server and client communicate through pipe",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const sockPath = tmpSockPath();
  const { promise, resolve } = Promise.withResolvers<string>();

  const server = net.createServer((conn) => {
    let data = "";
    conn.on("data", (chunk: Buffer) => data += chunk.toString());
    conn.on("end", () => {
      resolve(data);
      server.close();
    });
  });

  await new Promise<void>((r) => server.listen(sockPath, r));

  const client = net.connect(sockPath, () => {
    client.write("hello from pipe fd test");
    client.end();
  });

  const result = await promise;
  assertEquals(result, "hello from pipe fd test");

  try {
    fs.unlinkSync(sockPath);
  } catch {
    // ignore
  }
});

Deno.test({
  name: "unix socket bidirectional communication",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const sockPath = tmpSockPath();
  const { promise, resolve } = Promise.withResolvers<string>();

  const server = net.createServer((conn) => {
    conn.write("pong");
    conn.end();
  });

  await new Promise<void>((r) => server.listen(sockPath, r));

  const client = net.connect(sockPath, () => {
    let data = "";
    client.on("data", (chunk: Buffer) => data += chunk.toString());
    client.on("end", () => {
      resolve(data);
      server.close();
    });
  });

  const result = await promise;
  assertEquals(result, "pong");

  try {
    fs.unlinkSync(sockPath);
  } catch {
    // ignore
  }
});
