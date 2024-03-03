import { createRequire } from "module";

console.log(typeof createRequire(import.meta.url));
console.log(typeof createRequire(new URL(import.meta.url)));
console.log(typeof createRequire("/"));
console.log(typeof createRequire("/foo"));
console.log(typeof createRequire("/foo/"));
console.log(typeof createRequire("c:\\foo"));
try {
  createRequire("https://example.com/");
} catch (e) {
  console.log(e.message);
}
try {
  createRequire(new URL("https://example.com/"));
} catch (e) {
  console.log(e.message);
}
try {
  createRequire(1);
} catch (e) {
  console.log(e.message);
}
try {
  createRequire("foo");
} catch (e) {
  console.log(e.message);
}
try {
  createRequire("./foo");
} catch (e) {
  console.log(e.message);
}
