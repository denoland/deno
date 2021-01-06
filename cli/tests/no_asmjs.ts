// V8 logs any asmjs validation errors
// to stdout, but it shows line numbers
// that are non-existent in the source
// code, e.g.: JS/TS

const asm_js_module = function () {
  "use asm";

  type f64 = number;

  function add(
    x: f64,
  ): void {
    x = +x; // cast to float

    ~x;
    // asmjs error: `~` is only valid on integers
    // should not log to stdout with --no-validate-asm
  }

  return {
    add: add,
  };
}();
