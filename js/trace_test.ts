import { testPerm, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, async function traceFunctionSuccess() {
  const op = await deno.trace(async () => {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    // Mixing sync and async calls
    const filename = deno.makeTempDirSync() + "/test.txt";
    await deno.writeFile(filename, data, 0o666);
    await deno.removeSync(filename);
  });
  assertEqual(op.length, 3);
  assertEqual(op[0], { sync: true, name: "MakeTempDir" });
  assertEqual(op[1], { sync: false, name: "WriteFile" });
  assertEqual(op[2], { sync: true, name: "Remove" });
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
  const op = await deno.trace(promise);
  assertEqual(op.length, 3);
  assertEqual(op[0], { sync: true, name: "MakeTempDir" });
  assertEqual(op[1], { sync: false, name: "WriteFile" });
  assertEqual(op[2], { sync: true, name: "Remove" });
});

testPerm({ write: true }, async function traceRepeatSuccess() {
  const op1 = await deno.trace(async () => await deno.makeTempDir());
  assertEqual(op1.length, 1);
  assertEqual(op1[0], { sync: false, name: "MakeTempDir" });
  const op2 = await deno.trace(async () => await deno.statSync("."));
  assertEqual(op2.length, 1);
  assertEqual(op2[0], { sync: true, name: "Stat" });
});

testPerm({ write: true }, async function traceIdempotence() {
  let op1, op2, op3;
  op1 = await deno.trace(async () => {
    const filename = (await deno.makeTempDir()) + "/test.txt";
    op2 = await deno.trace(async () => {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      deno.writeFileSync(filename, data, 0o666);
      op3 = await deno.trace(async () => {
        await deno.remove(filename);
      });
      await deno.makeTempDir();
    });
  });

  // Flatten the calls
  assertEqual(op1.length, 4);
  assertEqual(op1[0], { sync: false, name: "MakeTempDir" });
  assertEqual(op1[1], { sync: true, name: "WriteFile" });
  assertEqual(op1[2], { sync: false, name: "Remove" });
  assertEqual(op1[3], { sync: false, name: "MakeTempDir" });

  assertEqual(op2.length, 3);
  assertEqual(op2[0], { sync: true, name: "WriteFile" });
  assertEqual(op2[1], { sync: false, name: "Remove" });
  assertEqual(op2[2], { sync: false, name: "MakeTempDir" });

  assertEqual(op3.length, 1);
  assertEqual(op3[0], { sync: false, name: "Remove" });

  // Expect top-level repeat still works after all the nestings
  const op4 = await deno.trace(async () => await deno.statSync("."));
  assertEqual(op4.length, 1);
  assertEqual(op4[0], { sync: true, name: "Stat" });
});
