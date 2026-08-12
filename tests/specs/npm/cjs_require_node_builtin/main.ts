import cjsBuiltin from "npm:@denotest/cjs-require-node-builtin@1.0.0";

if (cjsBuiltin.format("%s %s", "node", "builtins") !== "node builtins") {
  throw new Error('require("util") did not resolve from npm package');
}

if (cjsBuiltin.readableType !== "function") {
  throw new Error('require("stream") did not resolve from npm package');
}

console.log("resolved node builtins from npm package");
