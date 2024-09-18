console.error("starting serve");
export default {
  fetch(_req: Request) {
    console.error("serving request");
    return new Response("deno serve parallel");
  },
};
