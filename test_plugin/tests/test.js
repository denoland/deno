// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

const filenameBase = "test_plugin";

let filenameSuffix = ".so";
let filenamePrefix = "lib";

if (Deno.build.os === "windows") {
  filenameSuffix = ".dll";
  filenamePrefix = "";
}
if (Deno.build.os === "darwin") {
  filenameSuffix = ".dylib";
}

const filename = `../target/${
  Deno.args[0]
}/${filenamePrefix}${filenameBase}${filenameSuffix}`;

const resourcesPre = Deno.resources();

const pluginRid = Deno.openPlugin(filename);
console.log(`Plugin rid: ${pluginRid}`);

const {
  op_test_sync,
  op_test_async,
  op_test_resource_table_add,
  op_test_resource_table_get,
} = Deno.core.ops();

if (
  op_test_sync === null ||
  op_test_async === null ||
  op_test_resource_table_add === null ||
  op_test_resource_table_get === null
) {
  throw new Error("Not all expected ops were registered");
}

function runTestSync() {
  const result = Deno.core.opSync(
    "op_test_sync",
    { val: "1", map: { k: "v" } },
    new Uint8Array([116, 101, 115, 116]),
  );

  console.log(`op_test_sync returned: ${result}`);

  if (result !== "test") {
    throw new Error("op_test_sync returned an unexpected value!");
  }
}

async function runTestAsync() {
  const promise = Deno.core.opAsync(
    "op_test_async",
    { val: "1", map: { k: "v" } },
    new Uint8Array([49, 50, 51]),
  );

  if (!(promise instanceof Promise)) {
    throw new Error("Expected promise!");
  }

  const result = await promise;
  console.log(`op_test_async returned: ${result}`);

  if (result !== "test") {
    throw new Error("op_test_async promise resolved to an unexpected value!");
  }
}

function runTestResourceTable() {
  const expect = "hello plugin!";

  const testRid = Deno.core.opSync("op_test_resource_table_add", expect);
  console.log(`TestResource rid: ${testRid}`);

  if (testRid === null || Deno.resources()[testRid] !== "TestResource") {
    throw new Error("TestResource was not found!");
  }

  const testValue = Deno.core.opSync("op_test_resource_table_get", testRid);
  console.log(`TestResource get value: ${testValue}`);

  if (testValue !== expect) {
    throw new Error("Did not get correct resource value!");
  }

  Deno.close(testRid);
}

function runTestOpCount() {
  const start = Deno.metrics();

  Deno.core.opSync("op_test_sync", { val: "1", map: { k: "v" } });

  const end = Deno.metrics();

  if (end.opsCompleted - start.opsCompleted !== 1) {
    throw new Error("The opsCompleted metric is not correct!");
  }
  console.log("Ops completed count is correct!");

  if (end.opsDispatched - start.opsDispatched !== 1) {
    throw new Error("The opsDispatched metric is not correct!");
  }
  console.log("Ops dispatched count is correct!");
}

function runTestPluginClose() {
  // Closing does not yet work
  Deno.close(pluginRid);

  const resourcesPost = Deno.resources();

  const preStr = JSON.stringify(resourcesPre, null, 2);
  const postStr = JSON.stringify(resourcesPost, null, 2);
  if (preStr !== postStr) {
    throw new Error(
      `Difference in open resources before openPlugin and after Plugin.close():
Before: ${preStr}
After: ${postStr}`,
    );
  }
  console.log("Correct number of resources");
}

runTestSync();
await runTestAsync();
runTestResourceTable();

runTestOpCount();
// runTestPluginClose();
