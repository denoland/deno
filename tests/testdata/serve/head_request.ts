export default {
  fetch() {
    return new Response("hello world", {
      headers: { "content-type": "text/plain" },
    });
  },
};
