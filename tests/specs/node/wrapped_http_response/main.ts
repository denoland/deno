// Adapted from https://github.com/honojs/node-server/blob/1eb73c6d985665e75458ddd08c23bbc1dbdc7bcd/src/listener.ts
// and https://github.com/honojs/node-server/blob/1eb73c6d985665e75458ddd08c23bbc1dbdc7bcd/src/server.ts
import {
  buildOutgoingHttpHeaders,
  Response as WrappedResponse,
} from "./response.ts";
import { createServer, OutgoingHttpHeaders, ServerResponse } from "node:http";

Object.defineProperty(globalThis, "Response", {
  value: WrappedResponse,
});

const { promise, resolve } = Promise.withResolvers<void>();

const responseViaResponseObject = async (
  res: Response,
  outgoing: ServerResponse,
) => {
  const resHeaderRecord: OutgoingHttpHeaders = buildOutgoingHttpHeaders(
    res.headers,
  );

  if (res.body) {
    const buffer = await res.arrayBuffer();
    resHeaderRecord["content-length"] = buffer.byteLength;

    outgoing.writeHead(res.status, resHeaderRecord);
    outgoing.end(new Uint8Array(buffer));
  } else {
    outgoing.writeHead(res.status, resHeaderRecord);
    outgoing.end();
  }
};

const server = createServer((_req, res) => {
  const response = new Response("Hello, world!");
  return responseViaResponseObject(response, res);
});

using _server = {
  [Symbol.dispose]() {
    server.close();
  },
};

server.listen(0, async () => {
  const { port } = server.address() as { port: number };
  const response = await fetch(`http://localhost:${port}`);
  await response.text();
  resolve();
});

await promise;
console.log("done");
