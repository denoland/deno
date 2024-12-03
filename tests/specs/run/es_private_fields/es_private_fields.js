class Foo {
  #field = "field";

  setValue(val) {
    this.#field = val;
  }

  getValue() {
    return this.#field;
  }
}

const bar = new Foo();
bar.setValue("PRIVATE");
console.log(bar.getValue());
