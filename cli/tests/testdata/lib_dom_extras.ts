const { diagnostics, files } = await Deno.emit("/main.ts", {
  compilerOptions: {
    target: "esnext",
    lib: ["esnext", "dom"],
  },
  sources: {
    "/main.ts": `const as = new AbortSignal();
    console.log(as.reason);
    
    const up = new URLPattern("https://example.com/books/:id");
    console.log(up);
    `,
  },
});

console.log(diagnostics);
console.log(Object.keys(files).sort());
