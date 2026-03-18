// Test that stale keepAlive connections are transparently retried
// instead of throwing ECONNRESET. This matches Node.js behavior.
import http from "node:http";
import net from "node:net";

// Use a raw TCP server to have full control over connection lifecycle.
// It responds with HTTP/1.1 keep-alive, then closes the socket after idle.
let requestCount = 0;

const server = net.createServer((socket) => {
  let idleTimer: ReturnType<typeof setTimeout> | null = null;

  const resetIdle = () => {
    if (idleTimer) clearTimeout(idleTimer);
    idleTimer = setTimeout(() => socket.destroy(), 50);
  };

  socket.on("data", (_data) => {
    requestCount++;
    const body = `request ${requestCount}`;
    const response = [
      "HTTP/1.1 200 OK",
      `Content-Length: ${body.length}`,
      "Connection: keep-alive",
      "",
      body,
    ].join("\r\n");
    socket.write(response);
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

function makeRequest(path: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const req = http.get(
      {
        host: "localhost",
        port: (server.address() as net.AddressInfo).port,
        agent,
        path,
      },
      (res) => {
        let data = "";
        res.on("data", (chunk: Buffer) => (data += chunk));
        res.on("end", () => resolve(data));
      },
    );
    req.on("error", reject);
  });
}

server.listen(0, async () => {
  try {
    // First request - establishes connection
    const first = await makeRequest("/first");
    console.log("First request:", first);

    // Wait for server to close the idle connection (50ms timeout + buffer)
    await new Promise((resolve) => setTimeout(resolve, 150));

    // Second request - pooled socket is stale, should retry transparently
    const second = await makeRequest("/second");
    console.log("Second request:", second);

    // Third request to confirm continued operation
    const third = await makeRequest("/third");
    console.log("Third request:", third);

    console.log("All stale retry tests passed!");
  } catch (err) {
    console.error("FAILED:", (err as Error).message);
    Deno.exit(1);
  } finally {
    server.close();
    agent.destroy();
  }
});
