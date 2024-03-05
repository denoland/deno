const { addTrailers } = Deno[Deno.internal];

Deno.serve(async (req) => {
  const res = new Response("Hello World");
  addTrailers(res, { "X-Foo": "bar" });
  return res;
});
