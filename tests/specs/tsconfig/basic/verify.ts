const denoTsconfig = JSON.parse(Deno.readTextFileSync("deno.tsconfig.json"));
const co = denoTsconfig.compilerOptions;

// Check compiler options
console.log("strict:", co.strict);
console.log("module:", co.module);
console.log("moduleResolution:", co.moduleResolution);

// Check npm: paths
console.log("has npm:chalk path:", "npm:chalk" in (co.paths || {}));

// Check files includes deno types
const hasDenoTypes = denoTsconfig.files?.some((f: string) =>
  f.includes("deno.d.ts")
);
console.log("has deno types:", hasDenoTypes);

// Check tsconfig.json extends
const tsconfig = JSON.parse(Deno.readTextFileSync("tsconfig.json"));
console.log("extends:", tsconfig.extends);
