console.log("I am", import.meta.url, import.meta);
console.log("Resolving ./foo.js", await import.meta.resolve("./foo.js"));
console.log("Resolving ./foo.js from ./bar.js", await import.meta.resolve("./foo.js", "./bar.js"));

