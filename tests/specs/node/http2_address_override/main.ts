// node:http2 servers honor the DENO_SERVE_ADDRESS override, and HTTP/2 is
// forwarded natively (no downgrade): response trailers -- which only exist
// in HTTP/2 framing and are what gRPC rides on -- must survive the trip.
import http2 from "node:http2";

const server = http2.createServer();
server.on("stream", (stream, headers) => {
  let body = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk: string) => body += chunk);
  stream.on("end", () => {
    stream.respond({
      ":status": 200,
      "content-type": "application/grpc",
    }, { waitForTrailers: true });
    stream.on("wantTrailers", () => {
      stream.sendTrailers({ "grpc-status": "0", "grpc-message": "OK" });
    });
    stream.end(
      JSON.stringify({ path: headers[":path"], bodyLen: body.length }),
    );
  });
});

server.listen(0, () => {
  const client = http2.connect("http://127.0.0.1:12472");
  const stream = client.request({
    ":method": "POST",
    ":path": "/pkg.Service/Method",
  });

  let resHeaders: http2.IncomingHttpHeaders = {};
  let trailers: http2.IncomingHttpHeaders = {};
  let resBody = "";
  stream.on("response", (h) => resHeaders = h);
  stream.on("trailers", (t) => trailers = t);
  stream.setEncoding("utf8");
  stream.on("data", (chunk: string) => resBody += chunk);
  stream.on("end", () => {
    console.log("status:", resHeaders[":status"]);
    console.log("content-type:", resHeaders["content-type"]);
    console.log("body:", resBody);
    console.log("grpc-status:", trailers["grpc-status"]);
    console.log("grpc-message:", trailers["grpc-message"]);
    client.close();
    server.close(() => Deno.exit(0));
  });
  stream.end("grpc-frame-bytes");
});
