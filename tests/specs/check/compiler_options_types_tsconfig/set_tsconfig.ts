// Writes the root tsconfig for the requested variant: `direct` puts
// `compilerOptions.types` on the root tsconfig itself, `inherited` puts it
// on a base config the root extends.
const mode = Deno.args[0];
const types = ["@denotest/augments-global/import-meta"];
if (mode === "direct") {
  await Deno.writeTextFile(
    "tsconfig.json",
    JSON.stringify(
      { compilerOptions: { types }, files: ["main.ts"] },
      null,
      2,
    ),
  );
} else if (mode === "inherited") {
  await Deno.writeTextFile(
    "tsconfig.base.json",
    JSON.stringify({ compilerOptions: { types } }, null, 2),
  );
  await Deno.writeTextFile(
    "tsconfig.json",
    JSON.stringify(
      { extends: "./tsconfig.base.json", files: ["main.ts"] },
      null,
      2,
    ),
  );
} else {
  throw new Error(`unknown mode: ${mode}`);
}
