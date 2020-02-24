import { jsonTest as _jsonTest } from "./ops.js";
import { DispatchJsonPluginOp } from "./../../core/dispatch_json.js";

const jsonTest = new DispatchJsonPluginOp(_jsonTest);

const response = jsonTest.dispatchSync(
  { size: 12, name: "testObject" },
  new Uint8Array([116, 101, 115, 116])
);

console.log("Plugin Json Response:", response);
