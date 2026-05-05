// Verify that when there are no npm:/jsr: specifiers,
// no .deno/tsconfig.json or tsconfig.json is generated
try {
  Deno.statSync(".deno/tsconfig.json");
  console.log("deno tsconfig exists: true");
} catch {
  console.log("deno tsconfig exists: false");
}

try {
  Deno.statSync("tsconfig.json");
  console.log("root tsconfig exists: true");
} catch {
  console.log("root tsconfig exists: false");
}
