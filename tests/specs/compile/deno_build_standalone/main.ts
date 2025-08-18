const value: boolean = Deno.build.standalone;
console.log(value);

new Worker(import.meta.resolve("./worker.ts"), {
  "type": "module",
});
