async function doAFetch() {
  const resp = await fetch("http://localhost:4545/README.md");
  console.log(Deno.resources()); // print the current resources
  const _resp = resp;
  // at this point resp can be GC'ed
}

await doAFetch(); // create a resource

globalThis.gc(); // force GC

// It is very important that there is a yield here, otherwise the finalizer for
// the response body is not called and the resource is not closed.
await new Promise((resolve) => setTimeout(resolve, 0));

console.log(Deno.resources()); // print the current resources
