// Regression / feature test for https://github.com/denoland/deno/issues/XXXX
// https.Server.setTimeout() must fire a "timeout" event on the server when
// a TLS connection is idle (no HTTP data) for longer than the configured
// timeout duration, matching Node.js behaviour.

import * as https from "node:https";
import * as tls from "node:tls";
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

const server = https.createServer({ key, cert });

// Short timeout so the test completes quickly.
const TIMEOUT_MS = 300;

const timeoutFired = new Promise<void>((resolve) => {
  server.setTimeout(TIMEOUT_MS, (socket) => {
    console.log("timeout event fired");
    socket.destroy();
    resolve();
  });
});

await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
// deno-lint-ignore no-explicit-any
const { port } = server.address() as any;

// Open a raw TLS connection but never send any HTTP bytes so that the server's
// socket-idle timer triggers.
const clientSocket = tls.connect({
  host: "127.0.0.1",
  port,
  ca,
});

await new Promise<void>((resolve, reject) => {
  clientSocket.once("secureConnect", resolve);
  clientSocket.once("error", reject);
});

// Wait for the server to fire the timeout event.
await timeoutFired;

clientSocket.destroy();
server.close();

console.log("ok");
