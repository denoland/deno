// A simple long-running server used as the coverage target.
Deno.serve({ port: 0 }, (_req: Request) => {
  return new Response("Hello");
});
