export declare type HtmlEscaped = {
    isEscaped: true;
};
export declare type HtmlEscapedString = string & HtmlEscaped;
export declare type StringBuffer = [string];
export declare const escapeToBuffer: (str: string, buffer: StringBuffer) => void;
