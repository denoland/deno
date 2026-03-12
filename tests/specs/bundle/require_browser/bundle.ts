await Deno.bundle({
  entrypoints: ["./foo.cjs"],
  platform: "browser",
  outputPath: "bundle.js",
});
