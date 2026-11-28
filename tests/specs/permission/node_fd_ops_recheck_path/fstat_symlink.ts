import fs from "node:fs";

const fd = fs.openSync("link/secret.txt", "a");
try {
  console.log("fstatSync size:", fs.fstatSync(fd).size);
} catch (err) {
  console.log("fstatSync:", (err as Error).message);
}
fs.closeSync(fd);

using file = Deno.openSync("link/secret.txt", { append: true });
try {
  console.log("FsFile.statSync size:", file.statSync().size);
} catch (err) {
  console.log("FsFile.statSync:", (err as Error).message);
}
