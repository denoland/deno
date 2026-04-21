import http from "node:http";

const server = http.createServer((_req, res) => {
  res.writeHead(200);
  res.end("not a websocket");
});

server.on("upgrade", (req, nodeSocket, head) => {
  const { socket } = Deno.upgradeWebSocket(
    new Request("http://localhost/", { headers: req.headers }),
    { socket: nodeSocket, head },
  );

  socket.onopen = () => {
    console.log("server: ws open");
  };

  socket.onmessage = (e) => {
    console.log("server: received", e.data);
    socket.send("echo: " + e.data);
  };

  socket.onclose = () => {
    console.log("server: ws closed");
    server.close();
  };
});

server.listen(0, () => {
  const port = (server.address() as { port: number }).port;

  const ws = new WebSocket(`ws://localhost:${port}`);

  ws.onopen = () => {
    console.log("client: ws open");
    ws.send("hello");
  };

  ws.onmessage = (e) => {
    console.log("client: received", e.data);
    ws.close();
  };

  ws.onclose = () => {
    console.log("client: ws closed");
  };
});

// Safety timeout to prevent hanging on CI (unref'd so it doesn't keep the process alive)
const _t = setTimeout(() => {
  console.error("timeout - forcing exit");
  Deno.exit(1);
}, 60000);
Deno.unrefTimer(_t);
