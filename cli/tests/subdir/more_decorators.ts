// deno-lint-ignore-file
function a() {
  console.log("a(): evaluated");
  return (
    _target: any,
    _propertyKey: string,
    _descriptor: PropertyDescriptor,
  ) => {
    console.log("a(): called");
  };
}

export class B {
  @a()
  method() {
    console.log("method");
  }
}
