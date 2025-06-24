import { Duplex } from "node:stream";
import { connect } from "node:tls";
import { Socket } from "node:net";

class SocketWrapper extends Duplex {
  constructor(options) {
    super(options);
    this.socket = new Socket();
  }

  _write(chunk, encoding, callback) {
    this.socket.write(chunk, encoding, callback);
  }

  _read(size) {
  }

  connect(port, host) {
    this.socket.connect(port, host);
    this.socket.on("data", (data) => this.push(data));
    this.socket.on("end", () => this.push(null));
  }
}

const socketWrapper = new SocketWrapper();

socketWrapper.connect(443, "example.com");

const tlsSocket = connect({
  socket: socketWrapper,
  rejectUnauthorized: false,
});

tlsSocket.on("secureConnect", () => {
  console.log("TLS connection established");
  tlsSocket.write("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
});

tlsSocket.on("data", (data) => {
  console.log("Received:", data.toString());
  tlsSocket.end();
});

tlsSocket.on("end", () => {
  console.log("TLS connection closed");
});

tlsSocket.on("error", (error) => {
  console.error("Error:", error);
});
