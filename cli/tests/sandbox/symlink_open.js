// hello.txt is a symlink out of the sandbox.
// Expected to be run with cwd = tests/sandbox and allow-read=.
const f = Deno.openSync("hello.txt");
Deno.copy(f, Deno.stdout);
