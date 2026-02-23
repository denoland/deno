export declare type ParamMap = Array<[string, number]>;
export interface Context {
    varIndex: number;
}
export declare class Node {
    index?: number;
    varIndex?: number;
    children: Record<string, Node>;
    reverse: boolean;
    constructor({ reverse }?: Partial<Node>);
    newChildNode(): Node;
    insert(tokens: readonly string[], index: number, paramMap: ParamMap, context: Context): void;
    buildRegExpStr(): string;
}
