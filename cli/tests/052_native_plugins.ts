const filenameBase = "test_native_plugin";

let filenameSuffix = ".so";
let filenamePrefix = "lib";

if (Deno.build.os === "win") {
  filenameSuffix = ".dll";
  filenamePrefix = "";
}
if (Deno.build.os === "mac") {
  filenameSuffix = ".dylib";
}

const filename = `${filenamePrefix}${filenameBase}${filenameSuffix}`;

const plugin = Deno.openPlugin(`./../../target/${Deno.args[1]}/${filename}`);

// eslint-disable-next-line @typescript-eslint/camelcase
const test_io_sync = plugin.ops.test_io_sync;
// eslint-disable-next-line @typescript-eslint/camelcase
const test_io_async = plugin.ops.test_io_async;

const textDecoder = new TextDecoder();

{
  // eslint-disable-next-line @typescript-eslint/camelcase
  const response = test_io_sync.dispatch(
    new Uint8Array([116, 101, 115, 116]),
    new Uint8Array([116, 101, 115, 116])
  );

  console.log(`Native Binding Sync Response: ${textDecoder.decode(response)}`);
}

test_io_async.setAsyncHandler(response => {
  console.log(`Native Binding Async Response: ${textDecoder.decode(response)}`);
});

const response = test_io_async.dispatch(
  new Uint8Array([116, 101, 115, 116]),
  new Uint8Array([116, 101, 115, 116])
);

console.log(`Native Binding Async Response: ${textDecoder.decode(response)}`);
