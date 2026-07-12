// With an explicit --allow-read=inside.txt, the default cwd+temp confinement is
// not applied: the user's own read scope is not widened.

// The explicitly allowed file is readable.
Deno.readTextFileSync("./inside.txt");
console.log("read allowed file: ok");

// Another file in the same cwd is not, because the cwd was not auto-added.
try {
  Deno.readTextFileSync("./other.txt");
  console.log("read other cwd file: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log(`read other cwd file: ${err.name}`);
}
