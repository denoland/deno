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
  assertEqual(ops[0].name, "MakeTempDir");
  assertEqual(ops[0].sync, true);
  assertEqual(ops[1].name, "WriteFile");
  assertEqual(ops[1].sync, false);
  assertEqual(ops[2].name, "Remove");
  assertEqual(ops[2].sync, true);
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
  assertEqual(ops[0].name, "MakeTempDir");
  assertEqual(ops[0].sync, true);
  assertEqual(ops[1].name, "WriteFile");
  assertEqual(ops[1].sync, false);
  assertEqual(ops[2].name, "Remove");
  assertEqual(ops[2].sync, true);
});

testPerm({ write: true }, async function traceRepeatSuccess() {
  const ops1 = await deno.trace(async () => await deno.makeTempDir());
  assertEqual(ops1.length, 1);
  assertEqual(ops1[0].name, "MakeTempDir");
  assertEqual(ops1[0].sync, false);
  const ops2 = await deno.trace(async () => await deno.statSync("."));
  assertEqual(ops2.length, 1);
  assertEqual(ops2[0].name, "Stat");
  assertEqual(ops2[0].sync, true);
});

testPerm({ write: true }, async function traceIdempotence() {
  let ops1, ops2, ops3;
  ops1 = await deno.trace(async () => {
    const filename = (await deno.makeTempDir()) + "/test.txt";
    ops2 = await deno.trace(async () => {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      deno.writeFileSync(filename, data, 0o666);
      ops3 = await deno.trace(async () => {
        await deno.remove(filename);
      });
      await deno.makeTempDir();
    });
  });

  // Flatten the calls
  assertEqual(ops1.length, 4);
  assertEqual(ops1[0].name, "MakeTempDir");
  assertEqual(ops1[0].sync, false);
  assertEqual(ops1[1].name, "WriteFile");
  assertEqual(ops1[1].sync, true);
  assertEqual(ops1[2].name, "Remove");
  assertEqual(ops1[2].sync, false);
  assertEqual(ops1[3].name, "MakeTempDir");
  assertEqual(ops1[3].sync, false);

  assertEqual(ops2.length, 3);
  assertEqual(ops2[0].name, "WriteFile");
  assertEqual(ops2[0].sync, true);
  assertEqual(ops2[1].name, "Remove");
  assertEqual(ops2[1].sync, false);
  assertEqual(ops2[2].name, "MakeTempDir");
  assertEqual(ops2[2].sync, false);

  assertEqual(ops3.length, 1);
  assertEqual(ops3[0].name, "Remove");
  assertEqual(ops3[0].sync, false);

  // Expect top-level repeat still works after all the nestings
  const ops4 = await deno.trace(async () => await deno.statSync("."));
  assertEqual(ops4.length, 1);
  assertEqual(ops4[0].name, "Stat");
  assertEqual(ops4[0].sync, true);
});
