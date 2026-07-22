export default {
  fetch(request) {
    console.log(request.doesnt_exist);
    return new Response("Hello world!");
  },
} satisfies Deno.ServeDefaultExport;
