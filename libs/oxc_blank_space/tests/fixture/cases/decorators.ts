
@(Object.freeze as any)
//              as any
class A {}

@Object.freeze<any>export class B {}
//            <any>

export
@Object.freeze<any>
//            <any>
class C {}

@(Object.freeze<any>) declare class D {}
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

class E {
    @Object.freeze<any>
//                <any>
    field;

    @Object.freeze<any>
//                <any>
    private method() {}
//  private

    @(null as any)
//         as any
    accessor x;
}
