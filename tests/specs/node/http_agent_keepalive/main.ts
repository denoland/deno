// Test HTTP Agent keepAlive connection reuse
import http from "node:http";
import { Agent } from "node:http";
import { strictEqual } from "node:assert";

const agent = new Agent({
  keepAlive: true,
  maxSockets: 1,
  maxFreeSockets: 1,
});

const server = http.createServer((_req, res) => {
  res.end("ok");
});

function makeRequest(path: string): Promise<{ reusedSocket: boolean }> {
  return new Promise((resolve, reject) => {
    const req = http.get(
      {
        host: "localhost",
        port: (server.address() as { port: number }).port,
        agent,
        path,
      },
      (res) => {
        const reusedSocket = req.reusedSocket;
        res.on("data", () => {});
        res.on("end", () => {
          resolve({ reusedSocket });
        });
      },
    );
    req.on("error", reject);
  });
}

function waitForFreeSockets(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 10));
}

server.listen(0, async () => {
  const name = `localhost:${(server.address() as { port: number }).port}:`;

  // First request - should create new socket
  const first = await makeRequest("/first");
  strictEqual(
    first.reusedSocket,
    false,
    "First request should not reuse socket",
  );
  console.log("First request: reusedSocket =", first.reusedSocket);

  // Wait for socket to be returned to pool
  await waitForFreeSockets();
  strictEqual(
    agent.freeSockets[name]?.length,
    1,
    "Socket should be in freeSockets pool",
  );
  console.log("Socket in freeSockets pool:", agent.freeSockets[name]?.length);

  // Second request - should reuse socket
  const second = await makeRequest("/second");
  strictEqual(second.reusedSocket, true, "Second request should reuse socket");
  console.log("Second request: reusedSocket =", second.reusedSocket);

  // Wait for socket to be returned to pool
  await waitForFreeSockets();
  strictEqual(
    agent.freeSockets[name]?.length,
    1,
    "Socket should be back in freeSockets pool",
  );
  console.log(
    "Socket back in freeSockets pool:",
    agent.freeSockets[name]?.length,
  );

  // Third request - should still reuse socket
  const third = await makeRequest("/third");
  strictEqual(third.reusedSocket, true, "Third request should reuse socket");
  console.log("Third request: reusedSocket =", third.reusedSocket);

  console.log("All keepAlive tests passed!");

  server.close();
  agent.destroy();
});
