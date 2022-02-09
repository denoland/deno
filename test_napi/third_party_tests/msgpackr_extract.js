const msgpackr_extract = Deno.core.dlopen(
  "node_modules/msgpackr-extract/prebuilds/darwin-arm64/node.napi.glibc.node",
);

msgpackr_extract.extractStrings(new Uint8Array([0]));
