// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "../../std/http/server.ts";
import { assertEquals } from "../../std/testing/asserts.ts";

const addr = Deno.args[1] || "127.0.0.1:4555";
const dataSize = (1 << 26); // 64 MB

function generateData(size: number): string {
  const buffer = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    buffer[i] = i % 10;
  }
  return buffer.toString();
}

async function runServer(): Promise<void> {
  const server = serve(addr);

  console.log(`Proxy server listening on http://${addr}/`);
  for await (const req of server) {
    req.respond({ body: generateData(dataSize), status: 200 });
  }
}

async function testFetch(): Promise<void> {
  const response = await fetch(`http://${addr}`);
  const content = await response.text();
  const buffer = Uint8Array.from(content.split(",").map((x) => Number(x)));
  assertEquals(buffer.length, dataSize);
  for (let i = 0; i < buffer.length; i++) {
    assertEquals(buffer[i], i % 10);
  }
}

runServer();
await testFetch();
Deno.exit(0);
