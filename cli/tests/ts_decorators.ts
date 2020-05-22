function Decorate() {
    return function(constructor: T): any {
        return class extends constructor {
            protected someField: string = "asdf";
        }
    }
}

@Decorate()
class SomeClass {}

console.log((new SomeClass));