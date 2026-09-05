Deno.mkdirSync("node_modules/.deno", { recursive: true });
Deno.writeTextFileSync("node_modules/.deno/marker.txt", "keep\n");
