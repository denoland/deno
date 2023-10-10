console.log("Hello there 123!");

Deno.serve((req) => {
  console.log("request", req);
  return new Response("hello there 12");
});
