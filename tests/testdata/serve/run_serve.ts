Deno.serve((_req: Request) => {
  return new Response("Deno.serve() works!");
})