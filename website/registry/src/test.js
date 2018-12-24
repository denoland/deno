const { proxy, indexPage } = require("./app");
const assert = require("assert");

assert.equal(
  proxy("/x/net/main.js"),
  "https://raw.githubusercontent.com/denoland/deno_std/master/net/main.js"
);
assert.equal(
  proxy("/x/net/foo/bar.js"),
  "https://raw.githubusercontent.com/denoland/deno_std/master/net/foo/bar.js"
);
assert.equal(
  proxy("/x/net@v0.1.2/foo/bar.js"),
  "https://raw.githubusercontent.com/denoland/deno_std/v0.1.2/net/foo/bar.js"
);

console.log(indexPage());
