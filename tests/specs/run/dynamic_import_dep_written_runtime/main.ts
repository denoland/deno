Deno.writeTextFileSync("./b.ts", "console.log(1);");
await import("./a.ts");
Deno.writeTextFileSync("./b.ts", "console.log(2);");
