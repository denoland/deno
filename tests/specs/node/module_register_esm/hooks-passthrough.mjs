// A resolve hook that just calls nextResolve, which should produce the same
// URL that Deno's default resolver (including import maps) would produce.
export async function resolve(specifier, context, nextResolve) {
  return nextResolve(specifier, context);
}
