// V8 logs any asmjs validation errors to stdout, but it shows line numbers that
// are non-existent in the source.

const asmJsModule = function () {
  "use asm";
  function add(x) {
    x = +x; // cast to float

  }
  return { add };
  // asmjs error: compund object literal syntax isn't allowed
  // should not log to stdout with --no-validate-asm
}();
