let x /**/        /**/ = 1 ;
//        ^^^^^^^^        ^

[]                   ;
// ^^^^^^^^^^^^^^^^^^

class C /**/     /*︎*/ extends Array/**/    /*︎*/              /*︎*/ {
//          ^^^^^                      ^^^     ^^^^^^^^^^^^^^
             field/**/        /**/ = "";
//  ^^^^^^^^          ^^^^^^^^
    static accessor f1;
            f2/**/ /**/        /*︎*/;
//  ^^^^^^^       ^    ^^^^^^^^
                    
//  ^^^^^^^^^^^^^^^^ declared property

           method/**/   /*︎*/(/*︎*/        /**/ a  /*︎*/        /**/)/*︎*/      /*︎*/ {
//  ^^^^^^           ^^^         ^^^^^^^^      ^     ^^^^^^^^         ^^^^^^
    }

                       
//  ^^^^^^^^^^^^^^^^^^^ index signature

    get g()      { return 1 };
//         ^^^^^
    set g(v     ) { };
//         ^^^^^
}

class D extends C      {
//               ^^^^^
             method(...args)      {}
//  ^^^^^^^^                ^^^^^
}

class E extends (function() {}       ) {
//                             ^^^^^^
    d = C        ;
//       ^^^^^^^^
}

            class A {
// ^^^^^^^^
    ;          
//  ^^^^^^^^^^^ abstract property
    b;
                      
//  ^^^^^^^^^^^^^^^^^^ abstract method
}

{
    let m = new (Map )                ([] );
    //              ^ ^^^^^^^^^^^^^^^^   ^
}

{
    let a = (foo )     ;
    //          ^ ^^^^^
}

{
    let a = (foo )     ([] );
    //          ^ ^^^^^   ^
}

{
    let f = function(p     ) {}
    //                ^^^^^
}

{
                                
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ overload
    function overload()      {}
//                     ^^^^^
}

/** @doc */
;             
// ^^^^^^^^^^^ interface

void 0;

/** @doc */
           
// ^^^^^^^^ type alias


function foo   (p      = ()      => 1)      {
//          ^^^  ^^^^^     ^^^^^      ^^^^^
    return p       ;
//           ^^^^^^
}

/**/;                 
//  ^^^^^^^^^^^^^^^^^^ `declare enum`

void 0;

/**/                      
//  ^^^^^^^^^^^^^^^^^^^^^^ `declare namespace`

void 0;

/**/                       
//  ^^^^^^^^^^^^^^^^^^^^^^^ `declare module "path"`

void 0;

/**/                 
//  ^^^^^^^^^^^^^^^^^ `declare global {}`

void 0;

/**/              
//  ^^^^^^^^^^^^^^ `declare let`

void 0;

/**/                              
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `declare class`

void 0;

/**/                                          
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `declare function`

void 0;

// `=>` spanning line cases:
{
    ( 
       ) =>
//  :any =>
    1
};
{
    (  
      ) =>
//  any =>
    1
};
{
    (
     
       ) =>
//  :any =>
    1
};
{
    (
       
        
    )=>
//  )=>
    1
};
{
    (
      
                   
    )=>
//  >=>
    1
};
{
//» (a, b, c: D = [] as any/*comment-1*/)/*comment-2*/:
    (a, b, c    = []       /*comment-1*/               
      ) =>
//« any =>
    1
};
