import fs from "node:fs";

// a read-only fd doesn't require write permission
const fd = fs.openSync("target.txt", "r");
const time = new Date(0);

try {
  fs.futimesSync(fd, time, time);
  console.log("futimesSync: ok");
} catch (err) {
  console.log("futimesSync:", (err as Error).message);
}

await new Promise<void>((resolve) => {
  fs.futimes(fd, time, time, (err) => {
    console.log("futimes:", err ? err.message : "ok");
    resolve();
  });
});

console.log("mtime:", Deno.statSync("target.txt").mtime!.getTime() !== 0);
fs.closeSync(fd);
