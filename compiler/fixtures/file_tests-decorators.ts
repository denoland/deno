/* eslint-disable */

function enumerable(value: boolean) {
  return function (
    _target: any,
    _propertyKey: string,
    descriptor: PropertyDescriptor,
  ) {
    descriptor.enumerable = value;
  };
}

class A {
  @enumerable(false)
  a() {
    Test.value;
  }
}
