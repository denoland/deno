// Verify that an http import advertising X-TypeScript-Types gets its
// .d.ts mirrored and that the tsconfig paths entry points at it.
const tsconfig = JSON.parse(
  Deno.readTextFileSync(".deno/tsconfig.json"),
);
const paths = tsconfig.compilerOptions.paths ?? {};

const key = "http://localhost:4545/xTypeScriptTypes.ts";
const target = paths[key]?.[0];
console.log("has paths entry:", typeof target === "string");
console.log("target ends in .d.ts:", target?.endsWith(".d.ts") ?? false);

const localTypesPath = ".deno/remote/localhost/xTypeScriptTypes.d.ts";
try {
  const stat = Deno.statSync(localTypesPath);
  console.log("types mirror exists:", stat.isFile);
} catch {
  console.log("types mirror exists: false");
}

// The .ts JS source should NOT be mirrored — when types are present we
// skip the source mirror to avoid file/dir collisions.
try {
  Deno.statSync(".deno/remote/localhost/xTypeScriptTypes.ts");
  console.log("ts source mirror exists: true");
} catch {
  console.log("ts source mirror exists: false");
}
