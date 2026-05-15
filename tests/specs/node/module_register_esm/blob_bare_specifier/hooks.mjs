// Passthrough resolve hook. Registering it flips on `resolve_active` so
// every bare-specifier resolve goes through the hook bridge -- including
// bare specifiers whose parent is a `cannot-be-a-base` URL (blob:, data:).
// Calling `nextResolve` returns Deno's default resolution (import map etc.)
// rather than a naive `new URL(spec, parentURL)`, matching how transitive
// hooks like `@tailwindcss/postcss` interact with lume's MDX blob: modules.
// The hook runs in a worker thread, so we surface "I saw the bare specifier
// from a blob: parent" via stderr -- the spec test asserts on it.
export async function resolve(specifier, context, nextResolve) {
  if (
    specifier === "@mapped/foo" && context.parentURL?.startsWith("blob:")
  ) {
    process.stderr.write("[hook] saw bare from blob\n");
  }
  return nextResolve(specifier, context);
}
