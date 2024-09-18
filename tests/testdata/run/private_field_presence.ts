export class Person {
  #name: string;
  constructor(name: string) {
    this.#name = name;
  }

  equals(other: unknown) {
    return other &&
      typeof other === "object" &&
      #name in other &&
      this.#name === other.#name;
  }
}

const a = new Person("alice");
const b = new Person("bob");
const c = new Person("alice");

console.log(a.equals(b));
console.log(a.equals(c));
