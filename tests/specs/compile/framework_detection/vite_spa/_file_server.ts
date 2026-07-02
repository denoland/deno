// Local hermetic stand-in for `jsr:@std/http@^1/file-server` (remapped in
// `deno.json`) so the compile doesn't need network access. The generated
// entrypoint only imports `serveDir`; the real serving behavior is exercised by
// `@std/http`'s own tests.
export function serveDir(_req: Request): Promise<Response> {
  return Promise.resolve(new Response(null, { status: 404 }));
}
