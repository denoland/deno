// Verify that existing tsconfig.json gets "extends" added
const rootTsconfig = JSON.parse(
  Deno.readTextFileSync("tsconfig.json"),
);
console.log(
  "root extends deno:",
  rootTsconfig.extends === "./.deno/tsconfig.json",
);
// Existing compilerOptions should be preserved
console.log(
  "preserves strict:",
  rootTsconfig.compilerOptions?.strict === true,
);

// .deno/tsconfig.json should also exist
try {
  Deno.statSync(".deno/tsconfig.json");
  console.log("deno tsconfig exists: true");
} catch {
  console.log("deno tsconfig exists: false");
}
