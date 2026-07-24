const { pathToFileURL } = require("node:url");

const requiredEsm = require.resolve("./esm-entry.mjs");
const requiredEsmUrl = pathToFileURL(requiredEsm);

const loaded = require(requiredEsm);
console.log(loaded.default);

const loadedFromUrl = require(requiredEsmUrl.href);
console.log(loadedFromUrl.default);
