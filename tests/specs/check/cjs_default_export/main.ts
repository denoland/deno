import cjsDefault from "npm:@denotest/cjs-default-export";

// should error since cjsDefault.default() is a number
export const Test: string = cjsDefault.default();
