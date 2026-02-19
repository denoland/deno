import console from "node:console";
import crypto from "node:crypto";
import fs from "node:fs";

const pemBuffer = fs.readFileSync(
  new URL("../testdata/x509.pem", import.meta.url),
);

const x509 = new crypto.X509Certificate(pemBuffer);
console.log(x509);
