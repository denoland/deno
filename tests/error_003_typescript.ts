/* eslint-disable */
interface Value<T> {
  f?: (r: T) => any;
  v?: string;
}

interface C<T> {
  values?: (r: T) => Array<Value<T>>;
}

class A<T> {
  constructor(private e?: T, public s?: C<T>) {}
}

class B {
  t = "foo";
}

var a = new A(new B(), {
  values: o => [
    {
      v: o.t,
      f: x => "bar"
    }
  ]
});
