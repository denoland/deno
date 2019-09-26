// TODO In the future maybe we can extract the op id in the top-level and use a
// constant. But currently it's causing problems with snapshotting.

export function hello() {
  Deno.core.send(Deno.ops["hello"]);
}
