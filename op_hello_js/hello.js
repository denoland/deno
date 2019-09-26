const OP_HELLO = Deno.ops.add("hello");

export function hello() {
  Deno.core.send(OP_HELLO);
}
