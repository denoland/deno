// Verify that deno install generates tsconfig.deno.json correctly
const tsconfigDeno = JSON.parse(
  Deno.readTextFileSync("tsconfig.deno.json"),
);
const co = tsconfigDeno.compilerOptions;

// Check base compiler options
console.log("strict:", co.strict);
console.log("moduleResolution:", co.moduleResolution);
console.log("types includes deno:", (co.types || []).includes("deno"));

// Check npm: paths mapping
console.log("has npm:chalk path:", "npm:chalk" in (co.paths || {}));
console.log("has chalk alias path:", "chalk" in (co.paths || {}));

// Check tsconfig.json extends
const tsconfig = JSON.parse(Deno.readTextFileSync("tsconfig.json"));
console.log("extends:", tsconfig.extends);

// Check @types/deno exists
try {
  const stat = Deno.statSync("node_modules/@types/deno/index.d.ts");
  console.log("deno types exist:", stat.isFile);
} catch {
  console.log("deno types exist: false");
}
