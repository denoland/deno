console.log(Location);
console.log(Location.prototype);
console.log(location);
try {
  location.hostname = "bar";
} catch (error) {
  console.log(error.toString());
}
