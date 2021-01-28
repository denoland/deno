import "./global.ts";

//deno-lint-ignore no-undef
process.on("exit", () => {
  console.log(1);
});

function unexpected() {
  console.log(null);
}
//deno-lint-ignore no-undef
process.on("exit", unexpected);
//deno-lint-ignore no-undef
process.removeListener("exit", unexpected);

//deno-lint-ignore no-undef
process.on("exit", () => {
  console.log(2);
});
