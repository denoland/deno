export default {
  async fetch(request) {
    // console.log("Got request", request.url);
    return new Response("Hello world!");
  },
};
