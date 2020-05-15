import { jsonTest as _jsonTest, runTestPluginClose } from "./ops.js";
import { DispatchJsonOp } from "./../../core/dispatch_json.js";

class JsonError extends Error {
  constructor(msg) {
    super(msg);
    this.name = "JsonError";
  }
}

class ExpectedZeroCopy extends Error {
  constructor(msg) {
    super(msg);
    this.name = "ExpectedZeroCopy";
  }
}

function errorFactory(kind, msg) {
  switch (kind) {
    case 1:
      return new JsonError(msg);
    case 44:
      return new ExpectedZeroCopy(msg);
    default:
      return new Error(msg);
  }
}

const jsonTest = new DispatchJsonOp(_jsonTest, errorFactory);

function testJsonOp() {
  /// TODO(afinch7): add some async tests to this.
  try {
    jsonTest.dispatchSync(
      { name: "testObject" },
      new Uint8Array([116, 101, 115, 116])
    );
  } catch (err) {
    if (!err instanceof JsonError) {
      console.log(err);
    }
  }

  try {
    jsonTest.dispatchSync({ size: 12, name: "testObject" });
  } catch (err) {
    if (!err instanceof ExpectedZeroCopy) {
      console.log(err);
    }
  }

  const response = jsonTest.dispatchSync(
    { size: 12, name: "testObject" },
    new Uint8Array([116, 101, 115, 116])
  );

  console.log("Plugin Json Response:", response);
}

testJsonOp();
runTestPluginClose();
