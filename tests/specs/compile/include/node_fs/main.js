import fs from "node:fs";
import { access, readFile } from "node:fs/promises";
import { constants } from "node:fs";

const file = "data.txt";

// Test 1: fs.accessSync with F_OK
fs.accessSync(file, constants.F_OK);
console.log("accessSync F_OK: ok");

// Test 2: fs.accessSync with R_OK
fs.accessSync(file, constants.R_OK);
console.log("accessSync R_OK: ok");

// Test 3: fs.promises.access
await access(file, constants.R_OK);
console.log("access R_OK: ok");

// Test 4: fs.openSync + readSync
const fd = fs.openSync(file, "r");
const buf = Buffer.alloc(100);
const bytesRead = fs.readSync(fd, buf, 0, buf.length, 0);
console.log("openSync+readSync:", buf.toString("utf8", 0, bytesRead).trim());
fs.closeSync(fd);

// Test 5: fs.promises.readFile (already works, but verify)
const content = await readFile(file, "utf8");
console.log("readFile:", content.trim());

// Test 6: createReadStream
const streamContent = await new Promise((resolve, reject) => {
  let data = "";
  const stream = fs.createReadStream(file, { encoding: "utf8" });
  stream.on("data", (chunk) => (data += chunk));
  stream.on("end", () => resolve(data));
  stream.on("error", reject);
});
console.log("createReadStream:", streamContent.trim());
