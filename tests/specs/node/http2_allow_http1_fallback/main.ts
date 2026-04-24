// Regression test for https://github.com/denoland/deno/issues/33317
// `http2.createSecureServer({ allowHTTP1: true })` must handle HTTP/1.1
// clients. Previously the http2 connectionListener threw
// `ReferenceError: kIncomingMessage is not defined` as soon as an HTTP/1.1
// socket was dispatched to the fallback path, so the request never ran.

import * as http2 from "node:http2";
import * as https from "node:https";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);
const ca = readFileSync(
  new URL("../../../testdata/tls/RootCA.pem", import.meta.url),
);

const server = http2.createSecureServer(
  { allowHTTP1: true, cert, key },
  (req, res) => {
    console.log("request httpVersion:", req.httpVersion);
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("ok");
  },
);

await new Promise<void>((resolve) => server.listen(0, resolve));
const port = (server.address() as { port: number }).port;

const body = await new Promise<string>((resolve, reject) => {
  const req = https.request(
    { hostname: "localhost", port, path: "/", method: "GET", ca },
    (res) => {
      let data = "";
      res.setEncoding("utf8");
      res.on("data", (chunk) => (data += chunk));
      res.on("end", () => resolve(`${res.statusCode} ${data}`));
    },
  );
  req.on("error", reject);
  req.end();
});

console.log("client response:", body);
server.close();
