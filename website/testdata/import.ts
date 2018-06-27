import { A } from "http://site.com/foo";
import { B } from "http://site.com/bar"

export namespace X {
  export type A = number[];
}

export namespace Y {
  export function F(x: A) {
  
  }
  export namespace P {
    export function T(x: B) {
    
    }
    type B = null;
    export function G(x: B) {
    
    }
    export function F(x: A) {

    }
  }
  export function G(x: B) {

  }
}
