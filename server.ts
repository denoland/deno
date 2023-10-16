globalThis.state = { i: 0 };

function bar() {
}

function handler(req) {
  // console.log("req123", req);
  return new Response("hello124353");
}

Deno.serve(handler);
