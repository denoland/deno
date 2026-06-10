import * as http from "node:http";
import * as net from "node:net";

// Test that http.Server emits the "connect" event for CONNECT requests.
// This is essential for HTTP proxy servers (e.g., proxy-chain used by Crawlee).

// Start a simple TCP echo server to act as the "target"
const target = net.createServer((socket) => {
  socket.write("hello from target");
  socket.end();
});

target.listen(0, () => {
  const targetPort = (target.address() as net.AddressInfo).port;

  const server = http.createServer((_req, res) => {
    res.writeHead(200);
    res.end("ok");
  });

  server.on("connect", (req, clientSocket, _head) => {
    console.log(`CONNECT event received: ${req.method} ${req.url}`);

    // Connect to the target
    const [hostname, port] = req.url!.split(":");
    const targetSocket = net.connect(Number(port), hostname, () => {
      clientSocket.write(
        "HTTP/1.1 200 Connection Established\r\n\r\n",
      );
      targetSocket.pipe(clientSocket);
      clientSocket.pipe(targetSocket);
    });

    targetSocket.on("error", (err) => {
      clientSocket.end(`HTTP/1.1 502 Bad Gateway\r\n\r\n`);
    });
  });

  server.listen(0, () => {
    const proxyPort = (server.address() as net.AddressInfo).port;

    // Send a CONNECT request to the proxy
    const client = net.connect(proxyPort, "127.0.0.1", () => {
      client.write(
        `CONNECT 127.0.0.1:${targetPort} HTTP/1.1\r\nHost: 127.0.0.1:${targetPort}\r\n\r\n`,
      );
    });

    let data = "";
    client.on("data", (chunk) => {
      data += chunk.toString();
    });

    client.on("end", () => {
      // Should have received the 200 Connection Established + target data
      const lines = data.split("\r\n");
      console.log(`Client received status: ${lines[0]}`);
      if (data.includes("hello from target")) {
        console.log("Tunnel data received successfully");
      }
      client.end();
      server.close();
      target.close();
    });
  });
});
