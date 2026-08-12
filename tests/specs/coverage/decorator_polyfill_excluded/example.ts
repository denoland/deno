function Example() {
  return function (_target: unknown, _ctx: ClassDecoratorContext): void {
  };
}

@Example()
export class Foo {
  public something(): number {
    return 1;
  }
}
