// Tests that upgrading a destroyed socket throws a clear synchronous
// error. destroy() sets the `destroyed` flag synchronously, so the
// check is deterministic regardless of when the handle's close
// callback fires.

import http from "node:http";
import type { Socket } from "node:net";

const server = http.createServer();

server.on("upgrade", (req, nodeSocket, head) => {
  // Destroy the socket before upgrading — simulates the client
  // disconnecting between sending the upgrade request and the
  // server writing the 101 response.
  nodeSocket.destroy();

  try {
    Deno.upgradeWebSocket(
      new Request("http://localhost/", { headers: req.headers as HeadersInit }),
      { socket: nodeSocket as Socket, head },
    );
    console.log("ERROR: should have thrown");
  } catch (err) {
    console.log("upgrade rejected:", (err as Error).message);
  }
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
