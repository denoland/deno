const swc = Deno.core.napiOpen(
  "node_modules/@swc/core-darwin-arm64/swc.darwin-arm64.node",
);

console.log(
  await swc.parse(
    "1 + 1",
    Deno.core.encode('{ "syntax": "ecmascript" }'),
    "main.js",
  ),
);
