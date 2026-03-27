const { register } = require("node:module");
const { pathToFileURL } = require("node:url");

register("./loader_load.mjs", pathToFileURL(__filename));

const { value } = require("./target.mjs");
console.log("value:", value);
