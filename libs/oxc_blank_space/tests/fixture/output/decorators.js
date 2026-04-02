
@(Object.freeze       )
//              as any
class A {}

@Object.freeze     export class B {}
//            <any>

export
@Object.freeze     
//            <any>
class C {}

;                                       
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

class E {
    @Object.freeze     
//                <any>
    field;

    @Object.freeze     
//                <any>
            method() {}
//  private

    @(null       )
//         as any
    accessor x;
}
