async function main() {
  const root = `file://${Deno.cwd()}/`;
  await import(root + "tests/001_hello.js");
  // error occurred only for 2nd and following dynamic imports
  // import non-js file - it will throw SyntaxError
  await import(root + "README.md");
}

main();
