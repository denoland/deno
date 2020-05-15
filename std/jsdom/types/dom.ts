// TODO: Generate proper DOM types using https://github.com/microsoft/TSJS-lib-generator.

export namespace dom {
  export interface Window {
    document: Document;
  }

  export interface Document extends Node {
    body: HTMLElement;
    documentElement: HTMLElement;
  }

  export interface DocumentFragment extends Node {}

  export interface HTMLElement extends Node {
    children: HTMLElement[];
    outerHTML: string;
  }

  export interface Node {
    childNodes: ChildNode[];
    querySelector(query: string): HTMLElement;
    textContent: string | null;
  }

  export interface ChildNode extends Node {}
}
