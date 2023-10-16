globalThis.state = { i: 0 };

// function bar() {
// }

function handler(req) {
  console.log("req", req);
  return new Response("hello12");
}

Deno.serve(handler);
