const { files } = await Deno.emit("/mod.ts", {
  bundle: "classic",
  sources: {
    "/mod.ts": `import { hello } from "/hello.ts"; console.log(hello);`,
    "/hello.ts": `export const hello: string = "Hello, Compiler API!"`,
  },
  compilerOptions: {
    sourceMap: false,
  },
});

eval(files["deno:///bundle.js"]);
