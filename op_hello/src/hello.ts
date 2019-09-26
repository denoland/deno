const OP_HELLO = Deno.ops.add("hello");

export function hello() {
  Deno.send(OP_HELLO);
}
