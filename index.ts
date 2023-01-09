new Worker(import.meta.resolve("./worker.js"), { type: "module" });
await Deno.writeTextFile("./text.txt", "some malicious code");
