// Test that setting both ALPNCallback and ALPNProtocols throws
// ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS.

import tls from "node:tls";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

try {
  tls.createServer({
    cert,
    key,
    ALPNProtocols: ["h2"],
    ALPNCallback: () => "h2",
  });
  console.log("ERROR: should have thrown");
} catch (err) {
  console.log("caught:", err.code);
}
