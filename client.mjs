// client.mjs
import { connect } from "net";
import tls from "tls";
import { readFile } from "fs/promises";

const SOCKET_PATH = "/tmp/secure.sock";

const ca = await readFile("./server-cert.pem");

const rawSocket = connect(SOCKET_PATH); // net.Socket

const secureSocket = tls.connect({
  socket: rawSocket,
  rejectUnauthorized: false,
});

rawSocket.on("connect", () => {
  console.log("Connected to server");
});

secureSocket.on("secureConnect", () => {
  console.log("Secure connection to server");
  secureSocket.write("hello from client");
});

secureSocket.on("data", (data) => {
  console.log("Received from server:", data.toString());
});

secureSocket.on("error", (err) => {
  console.error("Error:", err);
});
