// Copyright 2018-2026 the Deno authors. MIT license.

const linker = Deno.args[0];
const outsidePath = linker === "isolated"
  ? "node_modules/.deno/@denotest+invalid-bin-target@1.0.0/node_modules/@denotest/outside.js"
  : "node_modules/@denotest/outside.js";

await Deno.mkdir(outsidePath.slice(0, outsidePath.lastIndexOf("/")), {
  recursive: true,
});
await Deno.writeTextFile(outsidePath, "outside marker\n");
if (Deno.build.os !== "windows") {
  await Deno.chmod(outsidePath, 0o644);
}
