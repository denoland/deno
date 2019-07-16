const { openPlugin, pluginFilename, env } = Deno;

const plugin = openPlugin(
  env().DENO_BUILD_PATH + "/" + pluginFilename("test_plugin")
);
const testOp = plugin.loadOp("test_op");
const asyncTestOp = plugin.loadOp("async_test_op");

interface TestOptions {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  zeroCopyData: any;
}

interface TestResponse {
  data: Uint8Array;
}

const textEncoder = new TextEncoder();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function encodeTestOp(args: any): Uint8Array {
  return textEncoder.encode(JSON.stringify(args));
}

const textDecoder = new TextDecoder();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function decodeTestOp(data: Uint8Array): string {
  return textDecoder.decode(data);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const doTestOp = (args: TestOptions): any => {
  const response = testOp.dispatch(
    encodeTestOp(args.data),
    encodeTestOp(args.zeroCopyData)
  );
  if (response instanceof Uint8Array) {
    return decodeTestOp(response);
  } else {
    throw new Error("Unexpected response type");
  }
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function doAsyncTestOp(args: TestOptions): Promise<any> {
  const response = asyncTestOp.dispatch(
    encodeTestOp(args.data),
    encodeTestOp(args.zeroCopyData)
  );
  if (response instanceof Promise) {
    return decodeTestOp(await response);
  } else {
    throw new Error("Unexpected response type");
  }
}

async function main(): Promise<void> {
  console.log(doTestOp({ data: "test", zeroCopyData: { some: "data" } }));
  console.log(
    await doAsyncTestOp({ data: "test", zeroCopyData: { some: "data" } })
  );
}

main();
