// Regression test: --deny-net=127.0.0.1 must block WebSocket connections
// whose hostname resolves to a denied IP, mirroring the post-resolution
// check that Deno.connect and fetch() already perform.

const server = Deno.serve(
  { hostname: "0.0.0.0", port: 0, onListen: () => {} },
  (req) => {
    const { socket, response } = Deno.upgradeWebSocket(req);
    socket.onmessage = (e) => socket.send(e.data);
    return response;
  },
);
const { port } = server.addr as Deno.NetAddr;

function tryConnect(url: string): Promise<"open" | "error"> {
  return new Promise((resolve) => {
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch {
      resolve("error");
      return;
    }
    ws.onopen = () => {
      ws.close();
      resolve("open");
    };
    ws.onerror = () => resolve("error");
  });
}

const direct = await tryConnect(`ws://127.0.0.1:${port}/`);
console.log(
  direct === "error"
    ? "PASS: ws to 127.0.0.1 denied"
    : "FAIL: ws to 127.0.0.1 was not denied",
);

const viaDns = await tryConnect(`ws://localhost:${port}/`);
console.log(
  viaDns === "error"
    ? "PASS: ws to localhost denied"
    : "FAIL: ws to localhost (resolves to denied IP) was not denied",
);

await server.shutdown();
