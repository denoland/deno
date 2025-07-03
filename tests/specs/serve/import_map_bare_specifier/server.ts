export default {
  fetch() {
    return new Response("Hello, world!");
  },
  onListen() {
    console.log("Listening");
    Deno.exit(0);
  },
} satisfies Deno.ServeDefaultExport;
