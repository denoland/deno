// Verify ERR_INVALID_FD_TYPE for non-pipe/tcp fds
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const net = require("net");
const fs = require("fs");
const path = require("path");
const os = require("os");

const tmpFile = path.join(os.tmpdir(), `deno_net_fd_test_${process.pid}.txt`);
fs.writeFileSync(tmpFile, "test");
const fileFd = fs.openSync(tmpFile, "r");
try {
  new net.Socket({ fd: fileFd });
  console.log("ERROR: should have thrown");
} catch (e: any) {
  console.log("expected error:", e.code);
} finally {
  fs.closeSync(fileFd);
  fs.unlinkSync(tmpFile);
}
