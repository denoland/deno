// A proxy receives its request target in absolute-form (RFC 9112 section
// 3.2.2). The server span must report it as `url.full` directly instead of
// appending it to the authority again, and still derive `url.path` /
// `url.query` from the origin-form portion.
import http from "node:http";
import net from "node:net";
import { text } from "node:stream/consumers";

const server = http.createServer((_req, res) => res.end("ok"));
await new Promise<void>((resolve) =>
  server.listen(0, "127.0.0.1", () => resolve())
);
const port = (server.address() as net.AddressInfo).port;

// Send an absolute-form request target by hand - node:http's client only
// produces one when an HTTP proxy is configured.
const socket = net.connect(port, "127.0.0.1");
const response = text(socket);
socket.write(
  "GET http://example.com/foo?a=1 HTTP/1.1\r\n" +
    `Host: 127.0.0.1:${port}\r\n` +
    "Connection: close\r\n\r\n",
);
await response;

server.close();
