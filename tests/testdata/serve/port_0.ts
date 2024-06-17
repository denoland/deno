export default {
  fetch(_req: Request) {
    return new Response("deno serve --port 0 works!");
  },
};
