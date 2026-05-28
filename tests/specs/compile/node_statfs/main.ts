import fs from "node:fs";
import assert from "node:assert/strict";

// `import.meta.filename` points at the embedded module inside the
// `deno compile` virtual file system. Calling node:fs.statfs on it used to
// fail because the implementation bypassed the FileSystem trait and called
// libc / `GetDiskFreeSpaceW` directly on a path that does not exist on disk.
const selfPath = import.meta.filename!;

// deno-lint-ignore no-explicit-any
function assertValidStatFs(result: any) {
  assert.equal(typeof result.type, "number");
  assert.equal(typeof result.bsize, "number");
  assert.equal(typeof result.blocks, "number");
  assert.equal(typeof result.bfree, "number");
  assert.equal(typeof result.bavail, "number");
  assert.equal(typeof result.files, "number");
  assert.equal(typeof result.ffree, "number");
}

// statfsSync on an embedded file in the VFS
assertValidStatFs(fs.statfsSync(selfPath));

// statfsSync with bigint
{
  const result = fs.statfsSync(selfPath, { bigint: true });
  assert.equal(typeof result.bsize, "bigint");
}

// statfs (callback)
{
  const result = await new Promise((resolve, reject) => {
    fs.statfs(selfPath, (err, stats) => err ? reject(err) : resolve(stats));
  });
  assertValidStatFs(result);
}

// fs.promises.statfs
{
  const result = await fs.promises.statfs(selfPath);
  assertValidStatFs(result);
}

console.log("success");
