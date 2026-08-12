// Tests that non-empty `head` bytes from the HTTP parser are correctly
// forwarded to the WebSocket. A raw TCP client sends a WebSocket frame
// immediately after the HTTP upgrade headers in a single write, so the
// HTTP parser puts the frame bytes into the `head` buffer.

import http from "node:http";
import net from "node:net";

// Build a masked WebSocket text frame (client→server frames must be masked).
function maskedTextFrame(text: string): Uint8Array {
  const payload = new TextEncoder().encode(text);
  const mask = new Uint8Array([0x12, 0x34, 0x56, 0x78]);
  const masked = new Uint8Array(payload.length);
  for (let i = 0; i < payload.length; i++) {
    masked[i] = payload[i] ^ mask[i % 4];
  }
  const frame = new Uint8Array(2 + 4 + payload.length);
  frame[0] = 0x81; // FIN + text opcode
  frame[1] = 0x80 | payload.length; // MASK bit + length (< 126)
  frame.set(mask, 2);
  frame.set(masked, 6);
  return frame;
}

const server = http.createServer();

server.on("upgrade", (req, nodeSocket, head) => {
  console.log(`server: head has ${head.length} bytes`);

  const { socket } = Deno.upgradeWebSocket(
    new Request("http://localhost/", { headers: req.headers as HeadersInit }),
    { socket: nodeSocket as net.Socket, head },
  );

  socket.onmessage = (e) => {
    console.log("server: received", e.data);
    // The raw client can't do the WebSocket close handshake,
    // so just shut down directly.
    server.close();
    setTimeout(() => Deno.exit(0), 200);
  };
});

server.listen(0, () => {
  const port = (server.address() as { port: number }).port;
  const wsKey = "dGhlIHNhbXBsZSBub25jZQ==";

  const upgradeReq =
    `GET / HTTP/1.1\r\nHost: localhost:${port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ${wsKey}\r\nSec-WebSocket-Version: 13\r\n\r\n`;

  const frame = maskedTextFrame("from-head");
  const reqBytes = new TextEncoder().encode(upgradeReq);
  const combined = new Uint8Array(reqBytes.length + frame.length);
  combined.set(reqBytes);
  combined.set(frame, reqBytes.length);

  const rawSocket = net.connect(port, "localhost", () => {
    // Single write: HTTP headers + WebSocket frame. The HTTP parser
    // consumes the headers; leftover frame bytes appear as `head`.
    rawSocket.write(combined);
  });
});

const _t = setTimeout(() => {
  console.error("timeout - forcing exit");
  Deno.exit(1);
}, 10_000);
Deno.unrefTimer(_t);
