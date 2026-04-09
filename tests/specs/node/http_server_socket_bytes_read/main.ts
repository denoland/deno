import http from "node:http";

// Test that req.socket.bytesRead is a number (not undefined) on HTTP server
// requests. Regression test for https://github.com/denoland/deno/issues/33090

const server = http.createServer((req, res) => {
  req.on("end", () => {
    const bytesRead = req.socket.bytesRead;
    console.log(`bytesRead type: ${typeof bytesRead}`);
    console.log(`bytesRead > 0: ${bytesRead > 0}`);
    res.end("ok");
    server.close();
  });
  req.resume();
});

server.listen(0, () => {
  const port = (server.address() as { port: number }).port;
  const req = http.request({ method: "PUT", port, hostname: "127.0.0.1" });
  req.write("hello");
  req.end();
});
