// Tests that upgrading a socket that was destroyed before the 101 write
// completes fires an error event on the WebSocket (not a crash/panic).

import http from "node:http";
import type { Socket } from "node:net";

const server = http.createServer();

server.on("upgrade", (req, nodeSocket, head) => {
  // Destroy the socket before upgrading — simulates the client
  // disconnecting between sending the upgrade request and the
  // server writing the 101 response.
  nodeSocket.destroy();

  const { socket } = Deno.upgradeWebSocket(
    new Request("http://localhost/", { headers: req.headers as HeadersInit }),
    { socket: nodeSocket as Socket, head },
  );

  socket.addEventListener("error", () => {
    console.log("ws: error event fired (expected)");
    server.close();
  });
});

server.listen(0, () => {
  const port = (server.address() as { port: number }).port;

  const ws = new WebSocket(`ws://localhost:${port}`);
  ws.onerror = () => {
    // Client will also error since the connection is destroyed
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
