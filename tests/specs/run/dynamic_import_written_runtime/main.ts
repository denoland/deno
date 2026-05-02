Deno.writeTextFileSync("./a.ts", "console.log(1);");
await import("./a.ts");
Deno.writeTextFileSync("./a.ts", "console.log(2);");
