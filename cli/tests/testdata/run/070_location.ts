// deno-lint-ignore-file no-global-assign
console.log(Location);
console.log(Location.prototype);
console.log(location);
try {
  location = {};
} catch (error) {
  if (error instanceof Error) {
    console.log(error.toString());
  }
}
try {
  location.hostname = "bar";
} catch (error) {
  if (error instanceof Error) {
    console.log(error.toString());
  }
}
