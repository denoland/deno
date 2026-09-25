export default {
  fetch(req) {
    return new Response("Hello world!");
  },
  // deno-lint-ignore no-explicit-any
  onError: 1 as any,
} satisfies Deno.ServeDefaultExport;
