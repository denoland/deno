import { testSync, testAsync } from "./ops.js";

const textDecoder = new TextDecoder();

function runTestSync() {
  const response = testSync.dispatch(
    new Uint8Array([116, 101, 115, 116]),
    new Uint8Array([116, 101, 115, 116])
  );

  console.log(`Plugin Sync Response: ${textDecoder.decode(response)}`);
}

testAsync.setAsyncHandler(response => {
  console.log(`Plugin Async Response: ${textDecoder.decode(response)}`);
});

function runTestAsync() {
  const response = testAsync.dispatch(
    new Uint8Array([116, 101, 115, 116]),
    new Uint8Array([116, 101, 115, 116])
  );

  if (response != null || response != undefined) {
    throw new Error("Expected null response!");
  }
}

function runTestOpCount() {
  const start = Deno.metrics();

  testSync.dispatch(new Uint8Array([116, 101, 115, 116]));

  const end = Deno.metrics();

  if (end.opsCompleted - start.opsCompleted !== 2) {
    // one op for the plugin and one for Deno.metrics
    throw new Error("The opsCompleted metric is not correct!");
  }
  if (end.opsDispatched - start.opsDispatched !== 2) {
    // one op for the plugin and one for Deno.metrics
    throw new Error("The opsDispatched metric is not correct!");
  }
}

runTestSync();
runTestAsync();

runTestOpCount();
