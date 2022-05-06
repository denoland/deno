Deno.core.print("import_inner_inner.js before\n");

await Deno.core.opAsync("op_sleep");

Deno.core.print("import_inner_inner.js after\n");

const abc = 1 + 2;
export function add(a, b) {
  Deno.core.print(`abc: ${abc}\n`);
  return a + b;
}
