// await import("https://deno.land/std/examples/catj.ts");
const deps = await Deno.deps("https://deno.land/std/examples/catj.ts");
console.log(deps);
