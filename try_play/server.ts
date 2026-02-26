const port = parseInt(Deno.env.get("PORT") || "3000");
console.error("DENO_COVERAGE_DIR:", Deno.env.get("DENO_COVERAGE_DIR"));

Deno.serve({ port }, (_req: Request) => {
  return new Response(
    `<!DOCTYPE html>
<html>
<head><title>Test App</title></head>
<body>
  <h1 id="greeting">Hello, Playwright!</h1>
  <button id="counter-btn">Clicked 0 times</button>
  <script>
    let count = 0;
    document.getElementById('counter-btn').addEventListener('click', () => {
      count++;
      document.getElementById('counter-btn').textContent = 'Clicked ' + count + ' times';
    });
  </script>
</body>
</html>`,
    { headers: { "content-type": "text/html" } },
  );
});
