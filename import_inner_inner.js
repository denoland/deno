await new Promise((_resolve) => {
});

const abc = 1 + 2;
export function add(a, b) {
  Deno.core.print(`abc: ${abc}\n`);
  return a + b;
}
