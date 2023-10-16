globalThis.state = { i: 0 };

function bar() {
}

function handler(req) {
  // console.log("req111123", req);
  return new Response("hello122334");
}

Deno.serve(handler);
