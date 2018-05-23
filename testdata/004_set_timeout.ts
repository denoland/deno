setTimeout(function() {
  console.log("World");
}, 10);

console.log("Hello");

const id = setTimeout(function() {
  console.log("Not printed");
}, 10000);

clearTimeout(id);
