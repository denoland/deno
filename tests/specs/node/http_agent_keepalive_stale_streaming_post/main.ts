// Test that streaming POST requests (using pipeline) on stale keepAlive
// connections are transparently retried with the request body intact.
// Streaming writes bypass outputData and go directly to _bodyWriter,
// so they need separate handling in the retry logic.
import http from "node:http";
import net from "node:net";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

let idleSocketClosed: Promise<void>;
let resolveIdleSocketClosed: () => void;
idleSocketClosed = new Promise((r) => (resolveIdleSocketClosed = r));

const BODY = '{"test":true}';

const server = net.createServer((socket) => {
  let idleTimer: ReturnType<typeof setTimeout> | null = null;

  const resetIdle = () => {
    if (idleTimer) clearTimeout(idleTimer);
    idleTimer = setTimeout(() => socket.destroy(), 50);
  };

  let buffer = "";
  socket.on("data", (data) => {
    buffer += data.toString();
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) return;

    const headers = buffer.substring(0, headerEnd);
    const clMatch = headers.match(/content-length:\s*(\d+)/i);
    const contentLength = clMatch ? parseInt(clMatch[1]) : 0;
    const bodyStart = headerEnd + 4;
    const body = buffer.substring(bodyStart, bodyStart + contentLength);

    if (body.length < contentLength) return;

    const responseBody = JSON.stringify({ received: body });
    const response = [
      "HTTP/1.1 200 OK",
      `Content-Length: ${responseBody.length}`,
      "Connection: keep-alive",
      "",
      responseBody,
    ].join("\r\n");
    socket.write(response);
    buffer = "";
    resetIdle();
  });

  socket.on("close", () => {
    if (idleTimer) clearTimeout(idleTimer);
    resolveIdleSocketClosed();
  });
});

const agent = new http.Agent({
  keepAlive: true,
  maxSockets: 1,
});

function postStreaming(port: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("timeout")), 5000);
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        method: "POST",
        agent,
        headers: { "Content-Length": String(BODY.length) },
      },
      (res) => {
        let data = "";
        res.on("data", (c: Buffer) => (data += c));
        res.on("end", () => {
          clearTimeout(timeout);
          resolve(data);
        });
      },
    );
    req.on("error", (e: Error) => {
      clearTimeout(timeout);
      reject(e);
    });

    const bodyStream = Readable.from(Buffer.from(BODY));
    pipeline(bodyStream, req).catch((e: Error) => {
      clearTimeout(timeout);
      reject(e);
    });
  });
}

server.listen(0, async () => {
  const port = (server.address() as net.AddressInfo).port;
  try {
    // First request - establishes keepAlive connection
    const first = await postStreaming(port);
    console.log("Request 1: OK", first);

    // Wait until the server closes the idle socket
    await idleSocketClosed;
    await new Promise((r) => setTimeout(r, 50));

    // Reset the idle close promise for the next connection
    idleSocketClosed = new Promise((r) => (resolveIdleSocketClosed = r));

    // Second request on stale socket - should retry with body intact
    const second = await postStreaming(port);
    console.log("Request 2: OK", second);

    console.log("PASS");
  } catch (e) {
    console.error("FAIL:", (e as Error).message);
    Deno.exit(1);
  } finally {
    server.close();
    agent.destroy();
  }
});
