// Verify that deno install generates .deno/tsconfig.json correctly
const tsconfigDeno = JSON.parse(
  Deno.readTextFileSync(".deno/tsconfig.json"),
);
const co = tsconfigDeno.compilerOptions;

// Check base compiler options
console.log("strict:", co.strict);
console.log("moduleResolution:", co.moduleResolution);
console.log("types includes deno:", (co.types || []).includes("deno"));

// Check npm: paths mapping
console.log("has npm:chalk path:", "npm:chalk" in (co.paths || {}));

// Check root tsconfig.json extends .deno/tsconfig.json
const rootTsconfig = JSON.parse(
  Deno.readTextFileSync("tsconfig.json"),
);
console.log(
  "root extends deno:",
  rootTsconfig.extends === "./.deno/tsconfig.json",
);

// Check @types/deno exists
try {
  const stat = Deno.statSync("node_modules/@types/deno/index.d.ts");
  console.log("deno types exist:", stat.isFile);
} catch {
  console.log("deno types exist: false");
}
