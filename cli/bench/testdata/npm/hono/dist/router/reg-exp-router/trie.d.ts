import type { ParamMap, Context } from './node';
import { Node } from './node';
export type { ParamMap } from './node';
export declare type ReplacementMap = number[];
interface InitOptions {
    reverse: boolean;
}
export declare class Trie {
    context: Context;
    root: Node;
    constructor({ reverse }?: InitOptions);
    insert(path: string, index: number): ParamMap;
    buildRegExp(): [RegExp, ReplacementMap, ReplacementMap];
}
