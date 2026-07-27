Deno.mkdirSync("dist", { recursive: true });
Deno.writeTextFileSync("dist/out.txt", "artifact");
console.log("generated");
