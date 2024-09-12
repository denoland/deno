console.error("starting serve");
export default {
  fetch(_req: Request) {
    console.log("serving request");
    return new Response("deno serve parallel");
  },
};
