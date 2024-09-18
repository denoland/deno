import { AlgorithmTypes } from './types';
export declare const sign: (payload: unknown, secret: string, alg?: AlgorithmTypes) => Promise<string>;
export declare const verify: (token: string, secret: string, alg?: AlgorithmTypes) => Promise<boolean>;
export declare const decode: (token: string) => {
    header: any;
    payload: any;
};
