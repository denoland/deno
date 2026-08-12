// Regression test for https://github.com/denoland/deno/issues/33367
// `https.createServer({ requestCert: true, ca, ... })` must actually ask the
// client for a certificate during the TLS handshake and surface it via
// `req.socket.getPeerCertificate()`. Previously the rustls ServerConfig was
// built with `with_no_client_auth()` unconditionally, so no CertificateRequest
// was sent and the peer certificate came back as `{}`.

import * as https from "node:https";
import { readFileSync } from "node:fs";

const localhostCert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const localhostKey = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);
const rootCa = readFileSync(
  new URL("../../../testdata/tls/RootCA.pem", import.meta.url),
);

const server = https.createServer(
  {
    key: localhostKey,
    cert: localhostCert,
    requestCert: true,
    ca: [rootCa],
  },
  (req, res) => {
    // deno-lint-ignore no-explicit-any
    const socket = req.socket as any;
    const peer = socket.getPeerCertificate();
    // deno-lint-ignore no-explicit-any
    console.log("authorized:", (req as any).client?.authorized);
    console.log("peer has subject:", !!peer && !!peer.subject);
    console.log("peer has issuer:", !!peer && !!peer.issuer);
    console.log("peer has fingerprint:", !!peer && !!peer.fingerprint);
    res.writeHead(200);
    res.end("ok");
    server.close();
  },
);

await new Promise<void>((resolve) => server.listen(0, resolve));
// deno-lint-ignore no-explicit-any
const port = (server.address() as any).port;

const status = await new Promise<number>((resolve, reject) => {
  const req = https.request(
    {
      hostname: "localhost",
      port,
      method: "GET",
      path: "/",
      cert: localhostCert,
      key: localhostKey,
      ca: rootCa,
      rejectUnauthorized: false,
    },
    (res) => {
      res.resume();
      res.on("end", () => resolve(res.statusCode ?? 0));
    },
  );
  req.on("error", reject);
  req.end();
});

console.log("status:", status);
