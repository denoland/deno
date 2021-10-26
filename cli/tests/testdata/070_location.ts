console.log(Location);
console.log(Location.prototype);
console.log(location);
try {
  location.hostname = "bar";
} catch (error) {
  if (error instanceof Error) {
    console.log(error.toString());
  }
}
