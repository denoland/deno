const abc = 1 + 2;

export function add(a, b) {
  Deno.core.print(`abc: ${abc}`);
  return a + b;
}
