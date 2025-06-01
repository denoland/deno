// V8 logs any asmjs validation errors to stdout, but it shows line numbers that
// are non-existent in the source.

const _asmJsModule = function () {
  "use asm";

  function func(
    x,
  ) {
    x = +x; // cast to float

    ~x;
    // asmjs error: `~` is only valid on integers
    // should not log to stdout with --no-validate-asm
  }

  return {
    f: func,
  };
}();
