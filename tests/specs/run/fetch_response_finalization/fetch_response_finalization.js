// Print the names of the currently open resources. The names, not the whole
// table: resource ids pack a slot index and a generation, so the numeric id a
// given resource ends up with depends on how many slots were recycled before
// it was opened. What this test is about is whether the `fetchResponse`
// resource disappears once the response is collected, and that is visible in
// the names alone.
function printResources() {
  const resources = Deno[Deno.internal].core.resources();
  console.log(Object.values(resources).join(", "));
}

async function doAFetch() {
  const resp = await fetch("http://localhost:4545/README.md");
  printResources();
  const _resp = resp;
  // at this point resp can be GC'ed
}

await doAFetch(); // create a resource

globalThis.gc(); // force GC

// It is very important that there is a yield here, otherwise the finalizer for
// the response body is not called and the resource is not closed.
await new Promise((resolve) => setTimeout(resolve, 0));

printResources();
