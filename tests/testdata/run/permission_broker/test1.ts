const _content = Deno.readTextFileSync("./README.md");
const _content_again = Deno.readTextFileSync("./README.md");
const _content2 = Deno.readTextFileSync("./Cargo.toml");

// Deno.writeTextFileSync("./scratch.txt", "Lorem ipsum dolor sit amet");

const server = Deno.serve(
  "http://localhost:12378",
  (req) => new Response("Hello world"),
);

console.log("env", Deno.env.toObject());

setTimeout(() => {
  server.shutdown();
}, 10_000);
