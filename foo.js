const lis = Deno.listen({port: 12345, hostname: 'localhost'})
console.log(`[srv] listening on http://localhost:${lis.addr.port}`)

for await (const conn of lis) {
  console.log("serveHttp");
  serveHttp(conn);
}

async function serveHttp(conn) {
  for await (const event of Deno.serveHttp(conn)) {
    event.respondWith(response(event.request))
  }
}

// Using at least one stream eventually leads to stuck connections.
// Making ALL bodies non-streams avoids stuck connections.
function response(req) {
  const path = new URL(req.url).pathname
  if (path === '/') {
    return new Response(html(), {headers: {'content-type': 'text/html'}})
  }
  if (path === '/main.css') {
    return new Response(css(), {headers: {'content-type': 'text/css'}})
  }
  throw Error(`unrecognized route ${path}`)
}

function html() {
  // return stream(
   return  `
<!doctype html>
<link rel="icon" href="data:;base64,=">
<link rel="stylesheet" type="text/css" href="/main.css">
`.trim()
// )
}

function css() {return ""}

// Quick and dirty way to make a readable stream from a string. Alternatively,
// `readableStreamFromReader(file)` could be used.
function stream(val) {
  return new Response(val).body
}
