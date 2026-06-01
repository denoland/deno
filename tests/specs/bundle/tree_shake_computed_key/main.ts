(() => {
  class A {
    *[Symbol.iterator]() {/* empty */}
  }
})();
