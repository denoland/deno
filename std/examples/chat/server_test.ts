// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../../testing/asserts.ts";
import { TextProtoReader } from "../../textproto/mod.ts";
import { BufReader } from "../../io/bufio.ts";
import { connectWebSocket, WebSocket } from "../../ws/mod.ts";
import { delay } from "../../async/delay.ts";

async function startServer(): Promise<
  Deno.Process<Deno.RunOptions & { stdout: "piped" }>
> {
  const server = Deno.run({
    // TODO(lucacasonato): remove unstable once possible
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-net",
      "--allow-read",
      "--unstable",
      "server.ts",
    ],
    cwd: "examples/chat",
    stdout: "piped",
  });
  try {
    assert(server.stdout != null);
    const r = new TextProtoReader(new BufReader(server.stdout));
    const s = await r.readLine();
    assert(s !== null && s.includes("chat server starting"));
  } catch (err) {
    server.stdout.close();
    server.close();
  }

  return server;
}

Deno.test({
  name: "[examples/chat] GET / should serve html",
  async fn() {
    const server = await startServer();
    try {
      const resp = await fetch("http://127.0.0.1:8080/");
      assertEquals(resp.status, 200);
      assertEquals(resp.headers.get("content-type"), "text/html");
      const html = await resp.text();
      assert(html.includes("ws chat example"), "body is ok");
    } finally {
      server.close();
      server.stdout.close();
    }
    await delay(10);
  },
});

Deno.test({
  name: "[examples/chat] GET /ws should upgrade conn to ws",
  async fn() {
    const server = await startServer();
    let ws: WebSocket | undefined;
    try {
      ws = await connectWebSocket("http://127.0.0.1:8080/ws");
      const it = ws[Symbol.asyncIterator]();

      assertEquals((await it.next()).value, "Connected: [1]");
      ws.send("Hello");
      assertEquals((await it.next()).value, "[1]: Hello");
    } finally {
      server.close();
      server.stdout.close();
      ws!.conn.close();
    }
  },
});
