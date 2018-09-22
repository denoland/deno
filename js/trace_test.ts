import { testPerm, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, async function traceFunctionSuccess() {
  const ops = await deno.trace(async () => {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    // Mixing sync and async calls
    const filename = deno.makeTempDirSync() + "/test.txt";
    await deno.writeFile(filename, data, 0o666);
    await deno.removeSync(filename);
  });
  assertEqual(ops.length, 3);
  assertEqual(ops[0], "MakeTempDir");
  assertEqual(ops[1], "WriteFile");
  assertEqual(ops[2], "Remove");
});

testPerm({ write: true }, async function tracePromiseSuccess() {
  // Ensure we don't miss any send actions
  // (new Promise(fn), fn runs synchronously)
  const asyncFunction = async () => {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    // Mixing sync and async calls
    const filename = deno.makeTempDirSync() + "/test.txt";
    await deno.writeFile(filename, data, 0o666);
    await deno.removeSync(filename);
  };
  const promise = Promise.resolve().then(asyncFunction);
  const ops = await deno.trace(promise);
  assertEqual(ops.length, 3);
  assertEqual(ops[0], "MakeTempDir");
  assertEqual(ops[1], "WriteFile");
  assertEqual(ops[2], "Remove");
});

testPerm({ write: true }, async function traceRepeatSuccess() {
  const ops1 = await deno.trace(async () => await deno.makeTempDir());
  assertEqual(ops1.length, 1);
  assertEqual(ops1[0], "MakeTempDir");
  const ops2 = await deno.trace(async () => await deno.statSync("."));
  assertEqual(ops2.length, 1);
  assertEqual(ops2[0], "Stat");
});
