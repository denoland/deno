
// Simple case
const a = async   (v   ) => {};
//             ^^^  ^^^

// Hard case - generic spans multiple lines
const b = async (
     
 /**/ /**/v   ) => {};
//   ^     ^^^

// Harder case - generic and return type spans multiple lines
const c = async (
     
  v              
                 
     
) => v;

// https://github.com/bloomberg/ts-blank-space/issues/29
(function () {
    return(  
         v   ) => v
});
(function () {
    return/**/(
         
     /**/ v         
    )/**/=> v
});
(function* () {
    yield(  
 v   )=>v;
});
(function* () {
    throw(  
 v   )=>v;
});
