const { register } = require("node:module");
const { pathToFileURL } = require("node:url");

register("./loader_initialize.mjs", {
  parentURL: pathToFileURL(__filename),
  data: { greeting: "hello from init" },
});

const { value } = require("./target.mjs");
console.log("value:", value);
