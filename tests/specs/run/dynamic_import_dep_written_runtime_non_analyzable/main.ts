Deno.writeTextFileSync("./b.ts", "console.log(1);");
const specifier = "./a.ts" + "";
await import(specifier); // b.ts will have a "missing" entry that needs to be purged when loading a.ts
await import("./b.ts");
