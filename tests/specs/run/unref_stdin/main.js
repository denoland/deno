const { core } = Deno[Deno.internal];
const opPromise = core.read(Deno.stdin.rid, new Uint8Array(10));
core.unrefOpPromise(opPromise);

async function main() {
  console.log(1);
  await opPromise;
  console.log(2);
}

main();
