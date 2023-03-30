import { iterateReader } from "https://deno.land/std@0.181.0/streams/iterate_reader.ts";

// Start listening on port 8080 of localhost.
const server = Deno.listen({ port: 8080 });
console.log(`HTTP webserver running.  Access it at:  http://localhost:8080/`);

// Connections to the server will be yielded up as an async iterable.
for await (const conn of server) {
  // In order to not be blocking, we need to handle each connection individually
  // without awaiting the function
  serveHttp(conn);
}

async function serveHttp(conn: Deno.Conn) {
  // This "upgrades" a network connection into an HTTP connection.
  const httpConn = Deno.serveHttp(conn);
  // Each request sent over the HTTP connection will be yielded as an async
  // iterator from the HTTP connection.
  for await (const requestEvent of httpConn) {
    if (requestEvent.request.headers.get("upgrade")) {
      let { request, respondWith } = requestEvent;
      console.log("upgrade!", request);
      let upgraded = await Deno.upgradeHttp2(request);
      console.log("upgraded", upgraded);
      let key = request.headers.get("sec-websocket-key");
      let response_key = Deno[Deno.internal].core.ops.op_http_websocket_accept_header(key);
      console.log("key = ", key, response_key);
      upgraded.write(Deno[Deno.internal].core.encode(
        `HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${response_key}\r\n\r\n`,
      )).await;
      for await (const chunk of iterateReader(upgraded)) {
        console.log(chunk);
        upgraded.write(chunk);
      }
    } else {
      console.log("request");
      // The native HTTP server uses the web standard `Request` and `Response`
      // objects.
      const body = `Your user-agent is:\n\n${
        requestEvent.request.headers.get("user-agent") ?? "Unknown"
      }`;
      // The requestEvent's `.respondWith()` method is how we send the response
      // back to the client.
      requestEvent.respondWith(
        new Response(body, {
          status: 200,
        }),
      );
    }
  }
}
