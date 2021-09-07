class Foo {
  constructor() {
    console.log("Foo");
  }

  invoke() {
    console.log("Foo.prototype.invoke");
  }
}

class Bar extends Foo {
  constructor() {
    super();

    console.log("Bar");
  }

  invoke() {
    super.invoke();

    console.log("Bar.prototype.invoke");
  }
}

class Baz extends Bar {
  constructor() {
    super();

    console.log("Baz");
  }

  invoke() {
    super.invoke();

    console.log("Baz.prototype.invoke");
  }
}

const bar = new Bar();
bar.invoke();
