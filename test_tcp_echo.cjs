const net = require("node:net");

const server = net.createServer((socket) => {
  socket.on("data", (data) => {
    const body = "Hello from Deno TCP (native libuv)!\n";
    const response = "HTTP/1.1 200 OK\r\n" +
      "Content-Type: text/plain\r\n" +
      "Content-Length: " + body.length + "\r\n" +
      "\r\n" +
      body;
    socket.write(response);
  });
  socket.on("error", () => {});
});

server.listen(8080, "127.0.0.1", () => {
  console.log("Server listening on http://127.0.0.1:8080");
});
