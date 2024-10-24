// deno-lint-ignore-file

function Decorate() {
  return function (constructor: any): any {
    return class extends constructor {
      protected someField: string = "asdf";
    };
  };
}

@Decorate()
class SomeClass {}

console.log(new SomeClass());
