// Tests that calling Deno.upgradeWebSocket twice on the same socket
// fails gracefully. The second upgrade should fire an error event
// because the TCP stream was already taken by the first upgrade.

import http from "node:http";
import type { Socket } from "node:net";

const server = http.createServer();

server.on("upgrade", (req, nodeSocket, head) => {
  const headers = req.headers as HeadersInit;

  // First upgrade — should succeed
  const { socket: ws1 } = Deno.upgradeWebSocket(
    new Request("http://localhost/", { headers }),
    { socket: nodeSocket as Socket, head },
  );

  // Second upgrade on the same node socket — should fail
  const { socket: ws2 } = Deno.upgradeWebSocket(
    new Request("http://localhost/", { headers }),
    { socket: nodeSocket as Socket, head },
  );

  ws1.onopen = () => console.log("ws1: open");
  ws1.onclose = () => {
    console.log("ws1: closed");
    server.close();
    // The second WebSocket's error leaves resources that prevent clean
    // shutdown, so exit explicitly after the close handshake completes.
    setTimeout(() => Deno.exit(0), 200);
  };

  ws2.addEventListener("error", () => {
    console.log("ws2: error (expected - stream already taken)");
  });
});

server.listen(0, () => {
  const port = (server.address() as { port: number }).port;

  const ws = new WebSocket(`ws://localhost:${port}`);
  ws.onopen = () => {
    console.log("client: open");
    ws.close();
  };
  ws.onclose = () => console.log("client: closed");
});

const _t = setTimeout(() => {
  console.error("timeout - forcing exit");
  Deno.exit(1);
}, 10_000);
Deno.unrefTimer(_t);
