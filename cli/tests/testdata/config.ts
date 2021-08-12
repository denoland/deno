// deno-lint-ignore-file

function b() {
  return function (
    _target: any,
    _propertyKey: string,
    _descriptor: PropertyDescriptor,
  ) {
    console.log("b");
  };
}

class A {
  @b()
  a() {
    console.log("a");
  }
}
