// Test that POST requests on stale keepAlive connections are transparently
// retried with the request body intact (not lost, which causes hangs).
// Regression test for https://github.com/denoland/deno/issues/32006
import http from "node:http";
import net from "node:net";

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

    // Extract content-length from headers
    const headers = buffer.substring(0, headerEnd);
    const clMatch = headers.match(/content-length:\s*(\d+)/i);
    const contentLength = clMatch ? parseInt(clMatch[1]) : 0;
    const bodyStart = headerEnd + 4;
    const body = buffer.substring(bodyStart, bodyStart + contentLength);

    if (body.length < contentLength) return; // wait for full body

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
  });
});

const agent = new http.Agent({
  keepAlive: true,
  maxSockets: 1,
});

function postRequest(port: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("timeout")), 5000);
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        method: "POST",
        agent,
        headers: { "Content-Length": "13" },
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
    req.write('{"test":true}');
    req.end();
  });
}

server.listen(0, async () => {
  const port = (server.address() as net.AddressInfo).port;
  try {
    // First POST request - establishes keepAlive connection
    const first = await postRequest(port);
    console.log("Request 1: OK", first);

    // Wait for server to close the idle connection
    await new Promise((r) => setTimeout(r, 150));

    // Second POST request on stale socket - should retry with body
    const second = await postRequest(port);
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
