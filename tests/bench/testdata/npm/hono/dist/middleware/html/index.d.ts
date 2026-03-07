import type { HtmlEscapedString } from '../../utils/html';
export declare const raw: (value: any) => HtmlEscapedString;
export declare const html: (strings: TemplateStringsArray, ...values: any[]) => HtmlEscapedString;
