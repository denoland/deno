// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Note: this is a keep-alive server.
const { Server } = require("net");
const port = process.argv[2] || "4544";
console.log("port", port);

const response = Buffer.from(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
);

Server((socket) => {
  socket.on("data", (_) => {
    socket.write(response);
  });
  socket.on("error", (_) => {
    socket.destroy();
  });
}).listen(port);
