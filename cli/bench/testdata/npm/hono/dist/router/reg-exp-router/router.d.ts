import type { Router, Result } from '../../router';
interface Hint {
    components: string[];
    regExpComponents: Array<true | string>;
    componentsLength: number;
    endWithWildcard: boolean;
    paramIndexList: number[];
    maybeHandler: boolean;
    namedParams: [number, string, string][];
}
interface HandlerWithSortIndex<T> {
    handler: T;
    index: number;
}
interface Route<T> {
    method: string;
    path: string;
    hint: Hint;
    handlers: HandlerWithSortIndex<T>[];
    middleware: HandlerWithSortIndex<T>[];
    paramAliasMap: Record<string, string[]>;
}
export declare class RegExpRouter<T> implements Router<T> {
    routeData?: {
        index: number;
        routes: Route<T>[];
        methods: Set<string>;
    };
    add(method: string, path: string, handler: T): void;
    match(method: string, path: string): Result<T> | null;
    private buildAllMatchers;
    private buildMatcher;
}
export {};
