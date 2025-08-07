import fs from "node:fs";
import tls from "node:tls";

// connect to google.com
const socket = tls.connect(443, "google.com", {
  rejectUnauthorized: false,
});

// handle the connection
socket.on("connect", () => {
  console.log("Connected to google.com");
  socket.write("GET / HTTP/1.1\r\nHost: google.com\r\nConnection: close\r\n\r\n");
});
