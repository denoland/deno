// deno-lint-ignore-file
function a() {
  console.log("@A evaluated");
  return function (
    target: any,
    propertyKey: string,
    descriptor: PropertyDescriptor,
  ) {
    console.log("@A called");
    const fn = descriptor.value;
    descriptor.value = function () {
      console.log("fn() called from @A");
      fn();
    };
  };
}

function b() {
  console.log("@B evaluated");
  return function (
    target: any,
    propertyKey: string,
    descriptor: PropertyDescriptor,
  ) {
    console.log("@B called");
    const fn = descriptor.value;
    descriptor.value = function () {
      console.log("fn() called from @B");
      fn();
    };
  };
}

class C {
  @a()
  @b()
  static test() {
    console.log("C.test() called");
  }
}

C.test();
