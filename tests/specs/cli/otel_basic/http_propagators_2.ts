Deno.serve({
  port: 8001,
}, (req) => {
  console.log("server 2");
  setTimeout(() => Deno.exit(0), 1000);
  return new Response("Hello World!");
});
