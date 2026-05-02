// Tests that upgrading a socket whose handle is gone throws a clear
// synchronous error (not a crash/panic).

import http from "node:http";
import type { Socket } from "node:net";

const server = http.createServer();

server.on("upgrade", (req, nodeSocket, head) => {
  // Save the handle so we can clean up afterwards
  const origHandle = (nodeSocket as Socket & { _handle: unknown })._handle;

  // Simulate a handle that was already cleared (e.g. socket closed
  // or handle taken by another path). This is deterministic unlike
  // nodeSocket.destroy() whose timing of nulling _handle varies.
  // deno-lint-ignore no-explicit-any
  (nodeSocket as any)._handle = null;

  try {
    Deno.upgradeWebSocket(
      new Request("http://localhost/", { headers: req.headers as HeadersInit }),
      { socket: nodeSocket as Socket, head },
    );
    console.log("ERROR: should have thrown");
  } catch (err) {
    console.log("upgrade rejected:", (err as Error).message);
  }

  // Restore handle so destroy() can properly close the TCP connection
  // deno-lint-ignore no-explicit-any
  (nodeSocket as any)._handle = origHandle;
  nodeSocket.destroy();
  server.close();
});

server.listen(0, () => {
  const port = (server.address() as { port: number }).port;

  const ws = new WebSocket(`ws://localhost:${port}`);
  ws.onerror = () => {
    // Client will error since the server never completes the upgrade
  };
  ws.onclose = () => {
    console.log("client: closed");
  };
});

const _t = setTimeout(() => {
  console.error("timeout - forcing exit");
  Deno.exit(1);
}, 10_000);
Deno.unrefTimer(_t);
