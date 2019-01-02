import { cwd, lstat, makeTempDirSync, removeAll, FileInfo } from "deno";
import { test, assert } from "../testing/mod.ts";
import { mkdirp } from "./mkdirp.ts";

let root: string = `${cwd()}/${Date.now()}`; //makeTempDirSync();

test(async function createsNestedDirs(): Promise<void> {
  const leaf: string = `${root}/levelx/levely`;
  await mkdirp(leaf);
  const info: FileInfo = await lstat(leaf);
  assert(info.isDirectory());
  await removeAll(root);
});

test(async function handlesAnyPathSeparator(): Promise<void> {
  const leaf: string = `${root}\\levelx\\levely`;
  await mkdirp(leaf);
  const info: FileInfo = await lstat(leaf.replace(/\\/g, "/"));
  assert(info.isDirectory());
  await removeAll(root);
});

test(async function failsNonDir(): Promise<void> {
  try {
    await mkdirp("./test.ts/fest.fs");
  } catch (err) {
    // TODO: assert caught DenoError of kind NOT_A_DIRECTORY or similar
    assert(err);
  }
});
