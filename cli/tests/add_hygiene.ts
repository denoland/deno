// deno-lint-ignore-file
function A() {
  console.log("@A evaluated");
  return function (
    target: any,
    propertyKey: string,
    descriptor: PropertyDescriptor,
  ) {
    console.log("@A called");
    const fn = descriptor.value;
    descriptor.value = function() {
      console.log("fn() called from @A");
      fn();  
    };
  };
}

function B() {
  console.log("@B evaluated");
  return function (
    target: any,
    propertyKey: string,
    descriptor: PropertyDescriptor,
  ) {
    console.log("@B called");
    const fn = descriptor.value;
    descriptor.value = function() {
      console.log("fn() called from @B");
      fn();  
    };
  };
}

class C {
  @A()
  @B()
  static test() {
    console.log("C.test() called");
  }
}

C.test();
