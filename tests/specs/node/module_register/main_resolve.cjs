const { register } = require("node:module");
const { pathToFileURL } = require("node:url");

register("./loader_resolve.mjs", pathToFileURL(__filename));

const { value } = require("my-virtual-module");
console.log("value:", value);
