type A = any;
type Box<T> = any;
//^^^^^^^^^^^^^^^^
declare const FOO: { [x: string]: <T>(...args: any[]) => any };
//^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

const {
    [(FOO as Box<any>).a]: a,
//       ^^^^^^^^^^^^
    [(FOO as Box<any>).b]: b,
//       ^^^^^^^^^^^^
    [(FOO as Box<any>).c]: c,
//       ^^^^^^^^^^^^
} = {} as any;
//    ^^^^^^^

const {
    data: {
        d,
        e,
        f,
    } = {} as Box<any>,
//        ^^^^^^^^^^^^
}: Box<any> = FOO || {};
//^^^^^^^^^

(function({
    name,
    regex,
    uuidType = String as Box<any>,
//                    ^^^^^^^^^^^
}: Box<any>) {
//^^^^^^^^^
});

let g: Box<any>,
    h!: Box<any>,
//   ^
    i: Box<any>;

(class {
    optionalMethod?(v: any) {}
//                ^
});

(function f0(
    this: any,
    //       ^- trailing comma
) {});

(function f1(
    this: any,
    //       ^- trailing comma
    arg1: any
) {});

({
    method() {
        return [FOO.cell/**/</*<*/boolean/*>*/>()]
//                          ^^^^^^^^^^^^^^^^^^^
            .map/**/</*<*/any/*>*/>(() => {})
//                  ^^^^^^^^^^^^^^^
    }
});

{
    function foo<T>(a: T) : T {
//              <T>  : T  : T
        return a;
    }

    class A {
        [foo<string>("")]<T>(a: T) {
//          <string>     <T>  : T
        }

        // @ts-expect-error: computed property names must have simple type
        [("A" + "B") as "AB"] =  1;
//                   as "AB"
    }
};

{
    (<T>(...args: any[]) => {})<any>`tagged ${"template" as any}`;
//   <T>        : any[]        <any>                     as any
};

{
    const obj = {
        [("A" + "B") as "AB"]: null
//                   as "AB"
    };
};
