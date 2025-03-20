export default {
  fetch(req: Request) {
    return new Response("Hello from declarative server");
  },
  onListen(info) {
    console.log(info);
  }
} satisfies Deno.ServeDefaultExport;
