import fs from "node:fs";
import tls from "node:tls";

const context = tls.createSecureContext({
  key: fs.readFileSync("key.pem"),
  cert: fs.readFileSync("cert.pem"),
});
