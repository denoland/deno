// Regression test for https://github.com/denoland/deno/issues/32590
// Coverage data should be written even when Deno.exit() is called.
function add(a, b) {
  return a + b;
}

function unused() {
  return "never called";
}

add(1, 2);
console.log("calling Deno.exit()");
Deno.exit(0);
