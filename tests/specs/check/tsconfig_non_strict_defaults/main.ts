// Assumes `"strictNullChecks": false`.
{
  const foo: string | null = eval('"foo"');
  foo.length;
}

// Assumes `"strictBindCallApply": false`.
{
  function fn1(x: string) {
    return console.log(x);
  }
  fn1.call(undefined, 1);
}

// Assumes `"strictBuiltinIteratorReturn": false`.
{
  const foo = [];
  const iterator = foo[Symbol.iterator]();
  const entry = iterator.next();
  if (entry.done) {
    const bar: string = entry.value;
    bar;
  }
}

// Assumes `"strictFunctionTypes": false`.
{
  function fn2(x: string) {
    console.log(x);
  }
  type StringOrNumberFunc = (ns: string | number) => void;
  const func: StringOrNumberFunc = fn2;
  func;
}

// Assumes `"strictPropertyInitialization": false`.
{
  class Foo {
    bar: string;
  }
  new Foo();
}

// Assumes `"noImplicitAny": false`.
{
  function fn3(s) {
    console.log(s.length);
  }
  fn3(1);
}

// Assumes `"noImplicitThis": false`.
{
  class Foo {
    bar: number = 1;
    method() {
      return function () {
        return this.bar;
      };
    }
  }
  new Foo();
}

// Assumes `"useUnknownInCatchVariables": false`.
{
  try {
    // ...
  } catch (err) {
    err.stack;
  }
}

// Assumes `"noImplicitOverride": false`.
{
  class Foo {
    method() {}
  }
  class Bar extends Foo {
    method() {}
  }
  new Bar();
}
