import fs from "node:fs";
import assert from "node:assert/strict";

// `import.meta.filename` points at the embedded module inside the
// `deno compile` virtual file system. Calling node:fs.fstatSync on a file
// descriptor opened against it used to fail with `NotSupported` because the
// VFS file's `stat_sync`/`stat_async` implementations were unimplemented.
const selfPath = import.meta.filename!;

// deno-lint-ignore no-explicit-any
function assertValidStat(stats: any) {
  assert.equal(stats.isFile(), true);
  assert.equal(stats.isDirectory(), false);
  assert.equal(typeof stats.size, "number");
  // the embedded module is not empty
  assert.equal(stats.size > 0, true);
  assert.equal(typeof stats.mtimeMs, "number");
}

// fstatSync on a descriptor for an embedded file in the VFS
{
  const fd = fs.openSync(selfPath, "r");
  try {
    assertValidStat(fs.fstatSync(fd));
  } finally {
    fs.closeSync(fd);
  }
}

// fstatSync with bigint
{
  const fd = fs.openSync(selfPath, "r");
  try {
    const stats = fs.fstatSync(fd, { bigint: true });
    assert.equal(typeof stats.size, "bigint");
    assert.equal(stats.isFile(), true);
  } finally {
    fs.closeSync(fd);
  }
}

// fstat (callback)
{
  const fd = fs.openSync(selfPath, "r");
  try {
    const stats = await new Promise((resolve, reject) => {
      fs.fstat(fd, (err, stats) => err ? reject(err) : resolve(stats));
    });
    assertValidStat(stats);
  } finally {
    fs.closeSync(fd);
  }
}

// fs.promises FileHandle.stat
{
  const handle = await fs.promises.open(selfPath, "r");
  try {
    assertValidStat(await handle.stat());
  } finally {
    await handle.close();
  }
}

console.log("success");
