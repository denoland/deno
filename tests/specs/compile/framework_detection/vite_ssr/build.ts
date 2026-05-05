await Deno.mkdir("dist", { recursive: true });
await Deno.writeTextFile("dist/index.html", "<h1>built</h1>");
