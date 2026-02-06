import * as net from "node:net";

const args = process.argv.slice(2);
let port = 0;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--port" && args[i + 1]) {
    port = Number.parseInt(args[i + 1], 10);
    i++;
  }
}

// Pick a random high port if port is 0, since some runtimes don't
// report the actual port from server.address() when listening on 0.
if (port === 0) {
  port = 10000 + Math.floor(Math.random() * 50000);
}

const server = net.createServer((socket) => {
  socket.pipe(socket);
});

server.listen(port, "127.0.0.1", () => {
  const actualPort = server.address().port || port;
  console.log(`ECHO_SERVER_READY ${actualPort}`);
});
