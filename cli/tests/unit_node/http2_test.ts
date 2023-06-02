import * as http2 from "node:http2";
import { deferred } from "../../../test_util/std/async/deferred.ts";
import { assertEquals } from "https://deno.land/std@v0.42.0/testing/asserts.ts";

const {
  HTTP2_HEADER_AUTHORITY,
  HTTP2_HEADER_METHOD,
  HTTP2_HEADER_PATH,
  HTTP2_HEADER_STATUS,
} = http2.constants;

Deno.test("[node/http2 fetch]", async () => {
  const portPromise = deferred();
  const reqPromise = deferred<Request>();
  const ready = deferred();
  const ac = new AbortController();
  const server = Deno.serve({
    port: 0,
    signal: ac.signal,
    onListen: ({ port }: { port: number }) => portPromise.resolve(port),
    handler: async (req: Request) => {
      console.log(req);
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
