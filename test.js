import ece from "npm:http_ece";
import { Buffer } from "node:buffer";
import crypto from "node:crypto";

const buf = Buffer.from("Hello, world!");
ece.encrypt(buf, {
  version: "aesgcm",
  key: crypto.randomBytes(16),
  rs: buf.length + 2 - 1,
});
