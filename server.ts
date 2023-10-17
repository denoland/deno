globalThis.state = { i: 0 };

function bar() {
}

function handler(req) {
  // console.log("req123", req);
  return new Response("Hello world!1234");
}

Deno.serve(handler);

addEventListener("hmr", (ev) => {
  console.log("event", ev.detail);
});
