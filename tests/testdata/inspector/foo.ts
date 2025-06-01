class Foo {
  hello(): string {
    return "hello";
  }
}

export function foo(): string {
  const f = new Foo();
  return f.hello();
}
