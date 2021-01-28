// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../../testing/asserts.ts";
import { TextProtoReader } from "../../textproto/mod.ts";
import { BufReader } from "../../io/bufio.ts";
import { delay } from "../../async/delay.ts";
import { dirname, fromFileUrl, resolve } from "../../path/mod.ts";

const moduleDir = resolve(dirname(fromFileUrl(import.meta.url)));

async function startServer(): Promise<
  Deno.Process<Deno.RunOptions & { stdout: "piped" }>
> {
  const server = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--quiet",
      "--allow-net",
      "--allow-read",
      "server.ts",
    ],
    cwd: moduleDir,
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
    let ws: WebSocket;
    try {
      ws = new WebSocket("ws://127.0.0.1:8080/ws");
      await new Promise<void>((resolve) => {
        ws.onmessage = ((message) => {
          assertEquals(message.data, "Connected: [1]");
          ws.onmessage = ((message) => {
            assertEquals(message.data, "[1]: Hello");
            ws.close();
            resolve();
          });
          ws.send("Hello");
        });
      });
    } catch (err) {
      console.log(err);
    } finally {
      server.close();
      server.stdout.close();
    }
  },
});
