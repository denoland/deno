export default {
  async fetch(_req) {
    return new Response("Hello world!");
  },
} satisfies Deno.ServeDefaultExport;

function checkNetPermission(host: string) {
  console.log(
    host,
    Deno.permissions.querySync({
      name: "net",
      host,
    }).state,
  );
}

checkNetPermission("github.com");
checkNetPermission("google.com");
checkNetPermission("localhost:1234");
checkNetPermission("localhost:12818");
Deno.exit(0);
