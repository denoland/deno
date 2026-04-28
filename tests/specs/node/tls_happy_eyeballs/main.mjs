// Regression test: TLS + Happy Eyeballs (autoSelectFamily).
// When a TLS connection falls back to a second address via Happy Eyeballs,
// kReinitializeHandle must re-wrap the new TCP handle with TLSWrap.
// Previously this caused "this._handle.start is not a function".
import https from "node:https";
import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const tmpDir = Deno.makeTempDirSync();
const keyPath = path.join(tmpDir, "key.pem");
const certPath = path.join(tmpDir, "cert.pem");

execSync(
  `openssl req -x509 -newkey rsa:2048 -keyout ${keyPath} -out ${certPath} -days 1 -nodes -subj "/CN=localhost" 2>/dev/null`,
);

const key = fs.readFileSync(keyPath);
const cert = fs.readFileSync(certPath);

// Listen ONLY on IPv6. When connecting to "localhost" (which resolves to
// both 127.0.0.1 and ::1), the IPv4 attempt fails with ECONNREFUSED,
// triggering Happy Eyeballs fallback to the IPv6 address.
const server = https.createServer({ key, cert }, (req, res) => {
  res.writeHead(200, { "Content-Type": "text/plain" });
  res.end("ok");
});

server.listen(0, "::1", () => {
  const port = server.address().port;

  const req = https.get(
    {
      hostname: "localhost",
      port,
      path: "/",
      rejectUnauthorized: false,
      agent: false,
    },
    (res) => {
      let body = "";
      res.on("data", (chunk) => (body += chunk));
      res.on("end", () => {
        console.log(`status: ${res.statusCode}`);
        console.log(`body: ${body}`);
        server.close();
      });
    },
  );

  req.on("error", (err) => {
    console.error(`error: ${err.message}`);
    server.close();
    process.exit(1);
  });
});
