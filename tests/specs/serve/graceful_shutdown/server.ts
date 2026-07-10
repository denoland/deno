export default {
  fetch(_req) {
    return new Response("ok");
  },
} satisfies Deno.ServeDefaultExport;
