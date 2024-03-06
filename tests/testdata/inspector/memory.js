const objs = [];

class Foo {
  foo() {
    return "foo";
  }
}

setInterval(() => {
  objs.push(new Foo());
}, 1000);

console.log("hello!");
