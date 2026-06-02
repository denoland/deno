// Writing through the `__proto__` accessor throws as well.
try {
  ({}).__proto__ = {};
  console.log("did not throw");
} catch (e) {
  console.log(e instanceof TypeError);
}
close();
