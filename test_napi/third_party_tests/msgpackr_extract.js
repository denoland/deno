const msgpackrExtract = Deno.core.napiOpen(
  "node_modules/msgpackr-extract/prebuilds/darwin-arm64/node.napi.glibc.node",
);

msgpackrExtract.extractStrings(new Uint8Array([0]));
