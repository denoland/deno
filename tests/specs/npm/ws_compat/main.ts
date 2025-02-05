import WebSocket from "npm:ws@8.18.0";

const key = Deno.readTextFileSync("../../../testdata/tls/localhost.key");
const cert = Deno.readTextFileSync("../../../testdata/tls/localhost.crt");

Deno.serve({ key, cert, port: 0, onListen }, (req) => {
  const { socket, response } = Deno.upgradeWebSocket(req);
  socket.addEventListener("open", () => {
    console.log("open on server");
  });
  socket.addEventListener("message", () => {
    console.log("message on server");
    Deno.exit(0);
  });
  return response;
});

function onListen({ port }) {
  const socket = new WebSocket(`wss://localhost:${port}`);
  socket.on("open", () => {
    console.log("open on client");
    socket.send("hi");
  });
}
