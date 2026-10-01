// A node:http request routed through a proxy writes the target host and port
// into the request it sends to the proxy (the absolute-form request line for an
// http proxy, the CONNECT request for an https one). A host or port carrying
// CR/LF must be refused with ERR_INVALID_CHAR before it reaches that wire,
// matching Node, rather than being injected. NODE_USE_ENV_PROXY=1 and
// HTTP_PROXY are set by the test runner.
//
// `setHost: false` skips the Host-header setter that would otherwise validate
// the host, so the raw value would reach the proxy socket unchecked without a
// check at the proxy boundary itself.

import http from "node:http";

function attempt(label, options) {
  try {
    const req = http.request(options);
    req.on("error", () => {});
    req.destroy();
    console.log(`${label}: no error`);
  } catch (e) {
    console.log(`${label}: ${e.code}`);
  }
}

attempt("crlf host", {
  host: "example.com\r\nX-Injected: 1",
  port: 80,
  setHost: false,
});
attempt("crlf port", {
  host: "example.com",
  port: "80\r\nX-Injected: 1",
  setHost: false,
});
attempt("valid host", { host: "127.0.0.1", port: 80, setHost: false });
