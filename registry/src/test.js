#!/usr/bin/env node

const { proxy, indexPage } = require("./app");
const assert = require("assert");

assert.equal(
  proxy("/x/net/main.js"),
  "https://raw.githubusercontent.com/denoland/deno_net/master/main.js"
);
assert.equal(
  proxy("/x/net/foo/bar.js"),
  "https://raw.githubusercontent.com/denoland/deno_net/master/foo/bar.js"
);
assert.equal(
  proxy("/x/net@v0.1.2/foo/bar.js"),
  "https://raw.githubusercontent.com/denoland/deno_net/v0.1.2/foo/bar.js"
);

// Just check that indexPage() doesn't crash and the body looks like html.
const r = indexPage();
assert(r.body.match(/html/i));

console.log("ok");
