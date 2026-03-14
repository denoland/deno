import * as http2 from "node:http2";
import * as net from "node:net";

const server = http2.createServer();

server.on("stream", (stream) => {
  stream.respond({ ":status": 200 });
  stream.end("ok");
});

server.listen(0, () => {
  const addr = server.address() as { port: number };

  const client = http2.connect(`http://127.0.0.1:${addr.port}`, {
    createConnection: () =>
      net.connect({ port: addr.port, host: "127.0.0.1" }),
  });

  client.on("error", (err: Error) => {
    console.error("client error:", err.message);
    Deno.exit(1);
  });

  const timer = setTimeout(() => {
    console.error("timeout");
    Deno.exit(1);
  }, 5000);

  const req = client.request({ ":path": "/" });
  req.resume();
  req.on("response", (headers: Record<string, string>) => {
    console.log(headers[":status"]);
  });
  req.on("end", () => {
    clearTimeout(timer);
    client.close();
    server.close();
  });
  req.end();
});
