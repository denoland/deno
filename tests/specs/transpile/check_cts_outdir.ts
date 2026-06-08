// Verify .cts files are transpiled to .cjs
const info = Deno.statSync("dist/helper.cjs");
console.log(info.isFile);
