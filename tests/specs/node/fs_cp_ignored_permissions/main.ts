import * as fs from "node:fs";
import * as fsPromises from "node:fs/promises";

// Test fs.cp with callback
await new Promise<void>((resolve, reject) => {
  fs.cp("dir1/data.txt", "dir2/data_cb.txt", (err) => {
    if (err) return reject(err);
    else resolve();
  });
});
console.log("callback: ok");

// Test fs.promises.cp
await fsPromises.cp("dir1/data.txt", "dir2/data_promise.txt");
console.log("promise: ok");

// Test fs.cpSync
fs.cpSync("dir1/data.txt", "dir2/data_sync.txt");
console.log("sync: ok");
