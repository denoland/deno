// TODO(ry) Once unit_tests.js lands (#448) this file should be removed
// and replaced with a faster version like was done in the prototype.
// https://github.com/denoland/deno/blob/golang/tests.ts#L34-L45
import * as deno from "deno";

const data = deno.readFileSync("package.json");
if (!data.byteLength) {
  throw Error(
    `Expected positive value for data.byteLength ${data.byteLength}`
  );
}
const decoder = new TextDecoder("utf-8");
const json = decoder.decode(data);
const pkg = JSON.parse(json);
if (pkg['devDependencies'] == null) {
  throw Error("Expected a positive number of devDependencies");
}
console.log("ok");
