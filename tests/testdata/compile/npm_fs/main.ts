import { url } from "npm:@denotest/esm-basic";
import { fileURLToPath } from "node:url";
import path from "node:path";
import assert from "node:assert/strict";

// will be at node_modules\.deno\@denotest+esm-basic@1.0.0\node_modules\@denotest\esm-basic
const dirPath = path.dirname(fileURLToPath(url));
const nodeModulesPath = path.join(dirPath, "../../../../../");
const packageJsonText = `{
  "name": "@denotest/esm-basic",
  "version": "1.0.0",
  "type": "module",
  "main": "main.mjs",
  "types": "main.d.mts"
}
`;
const vfsPackageJsonPath = path.join(dirPath, "package.json");

// reading a file in vfs
{
  const text = Deno.readTextFileSync(vfsPackageJsonPath);
  assert.equal(text, packageJsonText);
}

// reading a file async in vfs
{
  const text = await Deno.readTextFile(vfsPackageJsonPath);
  assert.equal(text, packageJsonText);
}

// copy file from vfs to real fs
{
  Deno.copyFileSync(vfsPackageJsonPath, "package.json");
  assert.equal(Deno.readTextFileSync("package.json"), packageJsonText);
}

// copy to vfs
assert.throws(
  () => Deno.copyFileSync("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
Deno.removeSync("package.json");

// copy file async from vfs to real fs
{
  await Deno.copyFile(vfsPackageJsonPath, "package.json");
  assert.equal(Deno.readTextFileSync("package.json"), packageJsonText);
}

// copy to vfs async
await assert.rejects(
  () => Deno.copyFile("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
Deno.removeSync("package.json");

// open
{
  const file = Deno.openSync(vfsPackageJsonPath);
  const bytes = new Uint8Array(10);
  file.seekSync(2, Deno.SeekMode.Start);
  assert.equal(file.readSync(bytes), 10);
  const text = new TextDecoder().decode(bytes);
  assert.equal(text, packageJsonText.slice(2, 12));
}
{
  const file = await Deno.open(vfsPackageJsonPath);
  const bytes = new Uint8Array(10);
  await file.seek(2, Deno.SeekMode.Start);
  assert.equal(await file.read(bytes), 10);
  const text = new TextDecoder().decode(bytes);
  assert.equal(text, packageJsonText.slice(2, 12));
}

// chdir
assert.throws(() => Deno.chdir(dirPath), Deno.errors.NotSupported);

// mkdir
assert.throws(
  () => Deno.mkdirSync(path.join(dirPath, "subDir")),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.mkdir(path.join(dirPath, "subDir")),
  Deno.errors.NotSupported,
);

// chmod
assert.throws(
  () => Deno.chmodSync(vfsPackageJsonPath, 0o777),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.chmod(vfsPackageJsonPath, 0o777),
  Deno.errors.NotSupported,
);

// chown
assert.throws(
  () => Deno.chownSync(vfsPackageJsonPath, 1000, 1000),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.chown(vfsPackageJsonPath, 1000, 1000),
  Deno.errors.NotSupported,
);

// remove
assert.throws(
  () => Deno.removeSync(vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.remove(vfsPackageJsonPath),
  Deno.errors.NotSupported,
);

// stat
{
  const result = Deno.statSync(vfsPackageJsonPath);
  assert(result.isFile);
}
{
  const result = await Deno.stat(vfsPackageJsonPath);
  assert(result.isFile);
}

// lstat
{
  const result = Deno.lstatSync(
    path.join(nodeModulesPath, "@denotest", "esm-basic"),
  );
  assert(result.isSymlink);
}
{
  const result = await Deno.lstat(
    path.join(nodeModulesPath, "@denotest", "esm-basic"),
  );
  assert(result.isSymlink);
}

// realpath
{
  const result = Deno.realPathSync(
    path.join(nodeModulesPath, "@denotest", "esm-basic", "package.json"),
  );
  assert.equal(result, vfsPackageJsonPath);
}
{
  const result = await Deno.realPath(
    path.join(nodeModulesPath, "@denotest", "esm-basic", "package.json"),
  );
  assert.equal(result, vfsPackageJsonPath);
}

// read dir
const readDirNames = ["main.d.mts", "main.mjs", "other.mjs", "package.json"];
{
  const names = Array.from(Deno.readDirSync(dirPath))
    .map((e) => e.name);
  assert.deepEqual(readDirNames, names);
}
{
  const names = [];
  for await (const entry of Deno.readDir(dirPath)) {
    names.push(entry.name);
  }
  assert.deepEqual(readDirNames, names);
}

// rename
assert.throws(
  () => Deno.renameSync("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
assert.throws(
  () => Deno.renameSync(vfsPackageJsonPath, "package.json"),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.rename("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.rename(vfsPackageJsonPath, "package.json"),
  Deno.errors.NotSupported,
);

// link
assert.throws(
  () => Deno.linkSync("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
assert.throws(
  () => Deno.linkSync(vfsPackageJsonPath, "package.json"),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.link("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.link(vfsPackageJsonPath, "package.json"),
  Deno.errors.NotSupported,
);

// symlink
assert.throws(
  () => Deno.symlinkSync("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
assert.throws(
  () => Deno.symlinkSync(vfsPackageJsonPath, "package.json"),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.symlink("package.json", vfsPackageJsonPath),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.symlink(vfsPackageJsonPath, "package.json"),
  Deno.errors.NotSupported,
);

// read link
{
  const result = Deno.readLinkSync(
    path.join(nodeModulesPath, "@denotest", "esm-basic"),
  );
  assert.equal(result, dirPath);
}
{
  const result = await Deno.readLink(
    path.join(nodeModulesPath, "@denotest", "esm-basic"),
  );
  assert.equal(result, dirPath);
}

// truncate
assert.throws(
  () => Deno.truncateSync(vfsPackageJsonPath, 0),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.truncate(vfsPackageJsonPath, 0),
  Deno.errors.NotSupported,
);

// utime
assert.throws(
  () => Deno.utimeSync(vfsPackageJsonPath, 0, 0),
  Deno.errors.NotSupported,
);
await assert.rejects(
  () => Deno.utime(vfsPackageJsonPath, 0, 0),
  Deno.errors.NotSupported,
);

console.log("success");
