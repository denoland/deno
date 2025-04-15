// @ts-expect-error "Deno[Deno.internal].core" is not a public interface
Deno[Deno.internal].core.evalContext(
  "(async () => console.log(await import('./_042_dyn_import_evalcontext/mod4.js')))()",
  new URL("..", import.meta.url).href,
);
