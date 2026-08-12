// Regression test for https://github.com/denoland/deno/issues/34585.
// Mirrors `react-remove-scroll-bar/constants`, whose nested
// `subpath/package.json` exposes a sibling file via `"main": "../dist/sub.js"`.
const subpath = require("@denotest/legacy-subpath-main/subpath");
console.log(subpath);
