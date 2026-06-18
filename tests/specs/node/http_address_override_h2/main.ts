// Control planes forward HTTP/2 requests (prior knowledge) to the
// DENO_SERVE_ADDRESS override listener, so node:http servers must accept
// both HTTP/1.1 and HTTP/2 connections there, like Deno.serve() does.
import { createServer } from "node:http";
import http2 from "node:http2";

const server = createServer((req, res) => {
  let body = "";
  req.on("data", (chunk) => body += chunk);
  req.on("end", () => {
    // node:http listeners must not observe HTTP/2 pseudo-headers, and
    // frameworks commonly copy req.headers into a web Headers object
    // (which rejects ":"-prefixed names) -- this must not throw.
    new Headers(req.headers as Record<string, string>);
    // http1 IncomingMessage parity: Object.prototype-backed and settable.
    if (typeof req.headers.hasOwnProperty !== "function") {
      throw new Error("req.headers must be Object.prototype-backed");
    }
    const savedHeaders = req.headers;
    req.headers = { replaced: "yes" };
    if (req.headers.replaced !== "yes") {
      throw new Error("req.headers must be assignable");
    }
    req.headers = savedHeaders;
    const savedRaw = req.rawHeaders;
    req.rawHeaders = [];
    if (req.rawHeaders.length !== 0) {
      throw new Error("req.rawHeaders must be assignable");
    }
    req.rawHeaders = savedRaw;
    const pseudo = Object.keys(req.headers).filter((k) => k[0] === ":");
    const rawPseudo = req.rawHeaders.filter((h, i) =>
      i % 2 === 0 && h[0] === ":"
    );
    res.setHeader("x-echo-method", req.method!);
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify({
      url: req.url,
      bodyLen: body.length,
      pseudo,
      rawPseudo,
      host: req.headers.host ?? null,
    }));
  });
});

async function doRequest(
  client: http2.ClientHttp2Session,
  headers: http2.OutgoingHttpHeaders,
  body: string | null,
): Promise<void> {
  const stream = client.request(headers);
  if (body !== null) {
    stream.write(body.slice(0, 5));
    stream.end(body.slice(5));
  } else {
    stream.end();
  }
  const resHeaders = await new Promise<http2.IncomingHttpHeaders>((resolve) =>
    stream.on("response", resolve)
  );
  let resBody = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk: string) => resBody += chunk);
  await new Promise((resolve) => stream.on("end", resolve));
  console.log("status:", resHeaders[":status"]);
  console.log("x-echo-method:", resHeaders["x-echo-method"]);
  console.log("body:", resBody);
}

server.listen(0, async () => {
  const client = http2.connect("http://127.0.0.1:12471");

  // POST with explicit content-length.
  await doRequest(client, {
    ":method": "POST",
    ":path": "/some/path?q=1",
    "content-length": "10",
  }, "0123456789");

  // POST without content-length: exercises the chunked request bridge.
  await doRequest(client, {
    ":method": "POST",
    ":path": "/chunked",
  }, "abcdefghijklmnop");

  // GET with no body.
  await doRequest(client, { ":method": "GET", ":path": "/get" }, null);

  client.close();
  server.close(() => Deno.exit(0));
});
