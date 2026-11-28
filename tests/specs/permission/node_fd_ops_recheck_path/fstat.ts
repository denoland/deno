import fs from "node:fs";

Deno.writeTextFileSync("secret.txt", "TOPSECRET-CONTENT\n");

// open write-only, which doesn't require read permission
const fd = fs.openSync("secret.txt", "a");

try {
  const stat = fs.fstatSync(fd);
  console.log("fstatSync size:", stat.size);
} catch (err) {
  console.log("fstatSync:", (err as Error).message);
}

await new Promise<void>((resolve) => {
  fs.fstat(fd, (err, stat) => {
    if (err) {
      console.log("fstat:", err.message);
    } else {
      console.log("fstat size:", stat.size);
    }
    resolve();
  });
});

try {
  const stat = await fs.promises.open("secret.txt", "a").then(async (fh) => {
    try {
      return await fh.stat();
    } finally {
      await fh.close();
    }
  });
  console.log("FileHandle.stat size:", stat.size);
} catch (err) {
  console.log("FileHandle.stat:", (err as Error).message);
}

fs.closeSync(fd);
