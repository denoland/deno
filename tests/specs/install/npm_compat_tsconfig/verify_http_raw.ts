// Verify that a plain .ts URL (no X-TypeScript-Types) is mirrored as-is
// and gets a tsconfig paths entry pointing at the local .ts file.
const tsconfig = JSON.parse(
  Deno.readTextFileSync(".deno/tsconfig.json"),
);
const paths = tsconfig.compilerOptions.paths ?? {};

const key = "http://localhost:4545/subdir/comment.ts";
const target = paths[key]?.[0];
console.log("has paths entry:", typeof target === "string");
console.log("target ends in .ts:", target?.endsWith(".ts") ?? false);

try {
  const stat = Deno.statSync(".deno/remote/localhost/subdir/comment.ts");
  console.log("source mirror exists:", stat.isFile);
} catch {
  console.log("source mirror exists: false");
}
