declare type FilePathOptions = {
    filename: string;
    root?: string;
    defaultDocument?: string;
};
export declare const getFilePath: (options: FilePathOptions) => string;
export {};
