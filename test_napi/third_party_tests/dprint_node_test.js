const dprint = Deno.core.napiOpen(
  "./node_modules/dprint-node/dprint-node.linux-x64-gnu.node",
);

console.log(
  dprint.format(
    "hello.js",
    "function x(){let a=1;return a;}",
    {
      lineWidth: 100,
      semiColons: "asi",
    },
  ),
);
