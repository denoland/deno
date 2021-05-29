const { code } = await Deno.emitBundle("/mod.ts", {
  type: "classic",
  sources: {
    "/mod.ts": `import { hello } from "/hello.ts"; console.log(hello);`,
    "/hello.ts": `export const hello: string = "Hello, Compiler API!"`,
  },
  compilerOptions: {
    sourceMap: false,
  },
});

eval(code);
