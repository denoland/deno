// Regression test for https://github.com/denoland/deno/issues/25792
// Aborting the signal passed to `Deno.serve()` must also close connections
// that were upgraded to websockets, otherwise the process never exits.
const controller = new AbortController();

const server = Deno.serve({
  port: 0,
  signal: controller.signal,
  onListen({ port }) {
    connect(port);
  },
}, (req) => {
  const { response, socket } = Deno.upgradeWebSocket(req);
  socket.onopen = () => console.log("server: ws open");
  socket.onclose = () => console.log("server: ws closed");
  return response;
});

function connect(port: number) {
  const ws = new WebSocket(`ws://127.0.0.1:${port}`);
  ws.onopen = () => {
    console.log("client: ws open");
    controller.abort();
  };
  ws.onclose = () => console.log("client: ws closed");
}

await server.finished;
console.log("server finished");

// Safety timeout (unref'd so it does not keep the process alive): if the
// websocket is not force closed it keeps the event loop alive forever and
// the test would hang without this.
const t = setTimeout(() => {
  console.error("timeout: websocket was not closed on abort");
  Deno.exit(1);
}, 60000);
Deno.unrefTimer(t);
