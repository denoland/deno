function sealed(ctor: Function) {
    Object.seal(ctor);
    Object.seal(ctor.prototype);
}

@sealed
export class Greeter {
    constructor (public greeting: string) {}
    greet() {
        return `Hello ${this.greeting}`;
    } 
}

console.log(Object.isSealed(Greeter));
