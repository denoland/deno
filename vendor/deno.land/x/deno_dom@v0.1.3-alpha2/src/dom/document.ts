import { setLock, getLock } from "../constructor-lock.ts";
import { Node, NodeType, Text, Comment } from "./node.ts";
import { NodeList, nodeListMutatorSym } from "./node-list.ts";
import { Element } from "./element.ts";
import { DOM as NWAPI } from "./nwsapi-types.ts";

export class DOMImplementation {
  constructor() {
    if (getLock()) {
      throw new TypeError("Illegal constructor.");
    }
  }

  createDocument() {
    throw new Error("Unimplemented"); // TODO
  }

  createHTMLDocument(titleStr?: string): HTMLDocument {
    titleStr += "";

    // TODO: Figure out a way to make `setLock` invocations less redundant
    setLock(false);
    const doc = new HTMLDocument();

    setLock(false);
    const docType = new DocumentType("html", "", "");
    doc.appendChild(docType);

    const html = new Element("html", doc, []);
    html._setOwnerDocument(doc);

    const head = new Element("head", html, []);
    const body = new Element("body", html, []);

    const title = new Element("title", head, []);
    const titleText = new Text(titleStr);
    title.appendChild(titleText);

    doc.head = head;
    doc.body = body;

    setLock(true);
    return doc;
  }

  createDocumentType(qualifiedName: string, publicId: string, systemId: string): DocumentType {
    setLock(false);
    const doctype = new DocumentType(qualifiedName, publicId, systemId);
    setLock(true);

    return doctype;
  }
}

export class DocumentType extends Node {
  #qualifiedName = "";
  #publicId = "";
  #systemId = "";

  constructor(
    name: string,
    publicId: string,
    systemId: string,
  ) {
    super(
      "html", 
      NodeType.DOCUMENT_TYPE_NODE, 
      null
    );

    this.#qualifiedName = name;
    this.#publicId = publicId;
    this.#systemId = systemId;
  }

  get name() {
    return this.#qualifiedName;
  }

  get publicId() {
    return this.#publicId;
  }

  get systemId() {
    return this.#systemId;
  }
}

export interface ElementCreationOptions {
  is: string;
}

export type VisibilityState = "visible" | "hidden" | "prerender";

export class Document extends Node {
  public head: Element = <Element> <unknown> null;
  public body: Element = <Element> <unknown> null;
  public implementation: DOMImplementation;

  #lockState = false;
  #documentURI = "about:blank"; // TODO
  #title = "";
  #nwapi = NWAPI(this);

  constructor() {
    super(
      (setLock(false), "#document"),
      NodeType.DOCUMENT_NODE,
      null,
    );

    setLock(false);
    this.implementation = new DOMImplementation();
    setLock(true);
  }

  // Expose the document's NWAPI for Element's access to
  // querySelector/querySelectorAll
  get _nwapi() {
    return this.#nwapi;
  }

  get documentURI() {
    return this.#documentURI;
  }

  get title() {
    return this.querySelector("title")?.textContent || "";
  }

  get cookie() {
    return ""; // TODO
  }

  set cookie(newCookie: string) {
    // TODO
  }

  get visibilityState(): VisibilityState {
    return "visible";
  }

  get hidden() {
    return false;
  }

  get compatMode(): string {
    return "CSS1Compat";
  }

  get documentElement(): Element | null {
    for (const node of this.childNodes) {
      if (node.nodeType === NodeType.ELEMENT_NODE) {
        return <Element> node;
      }
    }

    return null;
  }

  appendChild(child: Node) {
    super.appendChild(child);
    child._setOwnerDocument(this);
  }

  createElement(tagName: string, options?: ElementCreationOptions): Element {
    tagName = tagName.toUpperCase();

    setLock(false);
    const elm = new Element(tagName, null, []);
    elm._setOwnerDocument(this);
    setLock(true);
    return elm;
  }

  createTextNode(data?: string): Text {
    return new Text(data);
  }

  createComment(data?: string): Comment {
    return new Comment(data);
  }

  querySelector(selectors: string): Element | null {
    return this.#nwapi.first(selectors, this);
  }

  querySelectorAll(selectors: string): NodeList {
    const nodeList = new NodeList();
    const mutator = nodeList[nodeListMutatorSym]();
    mutator.push(...this.#nwapi.select(selectors, this))

    return nodeList;
  }

  // TODO: DRY!!!
  getElementById(id: string): Element | null {
    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        if ((<Element> child).id === id) {
          return <Element> child;
        }

        const search = (<Element> child).getElementById(id);
        if (search) {
          return search;
        }
      }
    }

    return null;
  }

  getElementsByTagName(tagName: string): Element[] {
    if (tagName === "*") {
      return this.documentElement
        ? <Element[]> this._getElementsByTagNameWildcard(this.documentElement, [])
        : [];
    } else {
      return <Element[]> this._getElementsByTagName(tagName.toUpperCase(), []);
    }
  }

  private _getElementsByTagNameWildcard(node: Node, search: Node[]): Node[] {
    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        search.push(child);
        (<any> child)._getElementsByTagNameWildcard(search);
      }
    }

    return search;
  }

  private _getElementsByTagName(tagName: string, search: Node[]): Node[] {
    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        if ((<Element> child).tagName === tagName) {
          search.push(child);
        }

        (<any> child)._getElementsByTagName(tagName, search);
      }
    }

    return search;
  }

  getElementsByTagNameNS(_namespace: string, localName: string): Element[] {
    return this.getElementsByTagName(localName);
  }

  getElementsByClassName(className: string): Element[] {
    return <Element[]> this._getElementsByClassName(className, []);
  }

  private _getElementsByClassName(className: string, search: Node[]): Node[] {
    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        if ((<Element> child).classList.contains(className)) {
          search.push(child);
        }

        (<any> child)._getElementsByClassName(className, search);
      }
    }

    return search;
  }

  hasFocus(): boolean {
    return true;
  }
}

export class HTMLDocument extends Document {
  constructor() {
    let lock = getLock();
    super();

    if (lock) {
      throw new TypeError("Illegal constructor.");
    }

    setLock(false);
  }
}

