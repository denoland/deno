Deno.writeTextFile("./mod.js", "console.log('hello')");
await import("./mod.js");
