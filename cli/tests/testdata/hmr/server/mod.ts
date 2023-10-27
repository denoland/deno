globalThis.state = { i: 0 };

function bar() {
  globalThis.state.i = 0;
  console.log("got request", globalThis.state.i);
}

function handler(_req) {
  bar();
  return new Response("Hello world!");
}

Deno.serve(handler);
