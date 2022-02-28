const url = new URL("./imported.js", import.meta.url);
await import("./imported.js");
