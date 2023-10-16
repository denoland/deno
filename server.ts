globalThis.state = { i: 0 };

function bar() {
}

function handler(req) {
  console.log("req", req);
  return new Response("hello1234");
}

Deno.serve(handler);
