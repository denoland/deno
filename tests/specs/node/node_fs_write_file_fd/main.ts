import { closeSync, openSync, readFileSync, writeFileSync } from "node:fs";
import { writeFile } from "node:fs/promises";

const tmp = Deno.makeTempDirSync();

// Test 1: writeFileSync with a numeric fd cleans up properly
{
  const path = `${tmp}/sync.txt`;
  Deno.writeTextFileSync(path, "");
  const fd = openSync(path, "w");
  writeFileSync(fd, "sync write via fd");
  // fd should still be usable after writeFileSync (not closed)
  writeFileSync(fd, "sync overwrite");
  closeSync(fd);
  console.log("sync:", readFileSync(path, "utf8"));
}

// Test 2: writeFile (async) with a numeric fd cleans up properly
{
  const path = `${tmp}/async.txt`;
  Deno.writeTextFileSync(path, "");
  const fd = openSync(path, "w");
  await writeFile(fd, "async write via fd");
  // fd should still be usable after writeFile
  await writeFile(fd, "async overwrite");
  closeSync(fd);
  console.log("async:", readFileSync(path, "utf8"));
}

// Test 3: writeFileSync with a path (string) still works
{
  const path = `${tmp}/path.txt`;
  writeFileSync(path, "via path");
  console.log("path:", readFileSync(path, "utf8"));
}

// Test 4: multiple writeFileSync calls with same fd don't leak
{
  const path = `${tmp}/multi.txt`;
  Deno.writeTextFileSync(path, "");
  const fd = openSync(path, "w");
  for (let i = 0; i < 10; i++) {
    writeFileSync(fd, `write ${i}`);
  }
  closeSync(fd);
  console.log("multi:", readFileSync(path, "utf8"));
}

Deno.removeSync(tmp, { recursive: true });
console.log("ok");
