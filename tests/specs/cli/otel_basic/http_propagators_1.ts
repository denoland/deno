Deno.serve(async () => {
  console.log("server 1");
  await fetch("http://localhost:8001");
  setTimeout(() => Deno.exit(0), 1000);
  return new Response("Hello World!");
});
