import * as http2 from "node:http2";
import * as net from "node:net";
import { deferred } from "../../../test_util/std/async/deferred.ts";
import { assertEquals } from "https://deno.land/std@v0.42.0/testing/asserts.ts";

const {
  HTTP2_HEADER_AUTHORITY,
  HTTP2_HEADER_METHOD,
  HTTP2_HEADER_PATH,
  HTTP2_HEADER_STATUS,
} = http2.constants;

Deno.test("[node/http2 client]", async () => {
  // Create a server to respond to the HTTP2 requests
  const portPromise = deferred();
  const reqPromise = deferred<Request>();
  const ready = deferred();
  const ac = new AbortController();
  const server = Deno.serve({
    port: 0,
    signal: ac.signal,
    onListen: ({ port }: { port: number }) => portPromise.resolve(port),
    handler: async (req: Request) => {
      reqPromise.resolve(req);
      await ready;
      return new Response("body", {
        status: 401,
        headers: { "resp-header-name": "resp-header-value" },
      });
    },
  });

  const port = await portPromise;

  // Get a session
  const sessionPromise = deferred();
  const session = http2.connect(
    `localhost:${port}`,
    {},
    sessionPromise.resolve.bind(sessionPromise),
  );
  const session2 = await sessionPromise;
  assertEquals(session, session2);

  // Write a request, including a body
  const stream = session.request({
    [HTTP2_HEADER_AUTHORITY]: `localhost:${port}`,
    [HTTP2_HEADER_METHOD]: "POST",
    [HTTP2_HEADER_PATH]: "/path",
    "req-header-name": "req-header-value",
  });
  stream.write("body");
  stream.end();

  // Check the request
  const req = await reqPromise;
  assertEquals(req.headers.get("req-header-name"), "req-header-value");
  assertEquals(await req.text(), "body");

  ready.resolve();

  // Read a response
  const headerPromise = new Promise<Record<string, string | string[]>>((
    resolve,
  ) => stream.on("headers", resolve));
  const headers = await headerPromise;
  assertEquals(headers["resp-header-name"], "resp-header-value");
  assertEquals(headers[HTTP2_HEADER_STATUS], "401");

  ac.abort();
  await server.finished;
});

Deno.test("[node/http2 server]", async () => {
  const server = http2.createServer();
  server.listen(0);
  const port = (<net.AddressInfo> server.address()).port;
  const sessionPromise = new Promise<http2.Http2Session>((resolve) =>
    server.on("session", resolve)
  );

  let responsePromise = fetch(`http://localhost:${port}/path`, {
    method: "POST",
    body: "body",
  });

  const session = await sessionPromise;
  const stream = await new Promise<http2.ServerHttp2Stream>((resolve) =>
    session.on("stream", resolve)
  );
  const headers = await new Promise((resolve) => stream.on("headers", resolve));
  const data = await new Promise((resolve) => stream.on("data", resolve));
  const end = await new Promise((resolve) => stream.on("end", resolve));
  stream.respond();
  stream.end();
  let resp = await responsePromise;
  await resp.text();

  await new Promise((resolve) => server.close(resolve));
});
