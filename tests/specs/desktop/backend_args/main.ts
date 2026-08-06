Deno.serve((req) => {
  const userAgent = req.headers.get("user-agent");
  if (userAgent) {
    console.log(userAgent);
    Deno.exit();
  } else {
    return new Response("", {
      headers: { "content-type": "text/html" },
    });
  }
});
