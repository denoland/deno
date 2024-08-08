import crypto from "node:crypto";
import fs from "node:fs";
import { Buffer } from "node:buffer";

const privateKey = crypto.createPrivateKey(fs.readFileSync("private.pem"));
const publicKey = crypto.createPublicKey(fs.readFileSync("public.pem"));

fs.writeFileSync("private2.pem", privateKey.export({ format: "pem", type: "pkcs8" }));

const signature = crypto.sign(null, Buffer.from("Hello, world!"), privateKey);
console.log(signature);
console.log(signature.toString("base64"), signature.byteLength);
const verified = crypto.verify(
  null,
  Buffer.from("Hello, world!"),
  publicKey,
  signature
);
console.log(verified);
