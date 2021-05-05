const { diagnostics, files } = await Deno.emit("/main.ts", {
  compilerOptions: {
    target: "esnext",
    lib: ["esnext", "dom", "dom.iterable", "dom.asynciterable"],
  },
  sources: {
    "/main.ts": `const rs = new ReadableStream<string>({
      start(c) {
        c.enqueue("hello");
        c.enqueue("deno");
        c.close();
      }
    });
    
    for await (const s of rs) {
      console.log("s");
    }
    `,
  },
});

console.log(diagnostics);
console.log(Object.keys(files).sort());
