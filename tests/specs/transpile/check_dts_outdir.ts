// Verify .d.ts files are placed in the outdir
const mainDts = Deno.statSync("dist/main.d.ts");
console.log(mainDts.isFile);
const helpersDts = Deno.statSync("dist/helpers.d.ts");
console.log(helpersDts.isFile);
