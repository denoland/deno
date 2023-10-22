globalThis.state = { i: 0 };

function bar() {
}

function handler(req) {
  console.log("req111123123", req);
  return new Response("hello4");
}

Deno.serve(handler);
