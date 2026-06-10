import type { StringBuffer, HtmlEscaped, HtmlEscapedString } from '../../utils/html';
declare global {
    namespace jsx.JSX {
        interface IntrinsicElements {
            [tagName: string]: Record<string, any>;
        }
    }
}
declare type Child = string | number | JSXNode | Child[];
export declare class JSXNode implements HtmlEscaped {
    tag: string | Function;
    props: Record<string, any>;
    children: Child[];
    isEscaped: true;
    constructor(tag: string | Function, props: Record<string, any>, children: Child[]);
    toString(): string;
    toStringToBuffer(buffer: StringBuffer): void;
}
export { jsxFn as jsx };
declare const jsxFn: (tag: string | Function, props: Record<string, any>, ...children: (string | HtmlEscapedString)[]) => JSXNode;
declare type FC<T = Record<string, any>> = (props: T) => HtmlEscapedString;
export declare const memo: <T>(component: FC<T>, propsAreEqual?: (prevProps: Readonly<T>, nextProps: Readonly<T>) => boolean) => FC<T>;
export declare const Fragment: (props: {
    key?: string;
    children?: any;
}) => JSXNode;
