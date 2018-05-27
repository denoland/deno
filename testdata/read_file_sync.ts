import { readFileSync } from "deno";

let data = readFileSync("package.json");
if (!data.byteLength) {
  throw Error(`Expected positive value for data.byteLength ${data.byteLength}`);
}

const decoder = new TextDecoder("utf-8");
const json = decoder.decode(data);
const pkg = JSON.parse(json);
if (pkg.name !== "deno") {
  throw Error(`Expected "deno" but got "${pkg.name}"`)
}
console.log("package.name ", pkg.name);
