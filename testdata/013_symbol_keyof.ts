const baz = Symbol("baz");

interface Thing {
    foo: string;
    bar: number;
    [baz]: boolean;
}

let x: keyof Thing = baz;

console.log(typeof x);