// Allocate memory until the heap limit is reached.
const arrays = [];
while (true) {
  arrays.push(new Array(1024 * 1024).fill(0));
}
