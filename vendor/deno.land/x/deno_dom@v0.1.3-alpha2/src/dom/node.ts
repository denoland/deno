import { getLock, setLock } from "../constructor-lock.ts";
import { NodeList, NodeListMutator, nodeListMutatorSym } from "./node-list.ts";
import { HTMLCollection, HTMLCollectionMutator, HTMLCollectionMutatorSym } from "./html-collection.ts";
import type { Element } from "./element.ts";
import type { Document } from "./document.ts";

export class EventTarget {
  addEventListener() {
    // TODO
  }

  removeEventListener() {
    // TODO
  }

  dispatchEvent() {
    // TODO
  }
}

export enum NodeType {
  ELEMENT_NODE = 1,
  ATTRIBUTE_NODE = 2,
  TEXT_NODE = 3,
  CDATA_SECTION_NODE = 4,
  ENTITY_REFERENCE_NODE = 5,
  ENTITY_NODE = 6,
  PROCESSING_INSTRUCTION_NODE = 7,
  COMMENT_NODE = 8,
  DOCUMENT_NODE = 9,
  DOCUMENT_TYPE_NODE = 10,
  DOCUMENT_FRAGMENT_NODE = 11,
  NOTATION_NODE = 12,
}

const nodesAndTextNodes = (nodes: (Node | any)[], parentNode: Node) => {
  return nodes.map(n => {
    let node = n;

    if (!(n instanceof Node)) {
      node = new Text("" + n);
    }

    node.parentNode = node.parentElement = parentNode;
    return node;
  });
}

export class Node extends EventTarget {
  public nodeValue: string | null;
  public childNodes: NodeList;
  public parentElement: Element | null;
  #childNodesMutator: NodeListMutator;
  #ownerDocument: Document | null = null;

  constructor(
    public nodeName: string,
    public nodeType: NodeType,
    public parentNode: Node | null,
  ) {
    super();
    if (getLock()) {
      throw new TypeError("Illegal constructor");
    }

    this.nodeValue = null;
    this.childNodes = new NodeList();
    this.#childNodesMutator = this.childNodes[nodeListMutatorSym]();
    this.parentElement = <Element> parentNode;

    if (parentNode) {
      parentNode.appendChild(this);
    }
  }

  _getChildNodesMutator(): NodeListMutator {
    return this.#childNodesMutator;
  }

  _setOwnerDocument(document: Document | null) {
    if (this.#ownerDocument !== document) {
      this.#ownerDocument = document;

      for (const child of this.childNodes) {
        child._setOwnerDocument(document);
      }
    }
  }

  get ownerDocument() {
    return this.#ownerDocument;
  }

  get textContent(): string {
    let out = "";

    for (const child of this.childNodes) {
      switch (child.nodeType) {
        case NodeType.TEXT_NODE:
          out += child.nodeValue;
          break;
        case NodeType.ELEMENT_NODE:
          out += child.textContent;
          break;
      }
    }

    return out;
  }

  set textContent(content: string) {
    for (const child of this.childNodes) {
      child.parentNode = child.parentElement = null;
    }

    this._getChildNodesMutator().splice(0, this.childNodes.length);
    this.appendChild(new Text(content));
  }

  cloneNode() {
    // TODO
  }

  remove() {
    const parent = this.parentNode;

    if (parent) {
      const nodeList = parent._getChildNodesMutator();
      const idx = nodeList.indexOf(this);
      nodeList.splice(idx, 1);
      this.parentNode = this.parentElement = null;
    }
  }

  appendChild(child: Node) {
    const oldParentNode = child.parentNode;

    // Check if we already own this child
    if (oldParentNode === this) {
      if (this.#childNodesMutator.indexOf(child) !== -1) {
        return;
      }
    } else if (oldParentNode) {
      child.remove();
    }

    child.parentNode = this;

    // If this a document node or another non-element node
    // then parentElement should be set to null
    if (this.nodeType === NodeType.ELEMENT_NODE) {
      child.parentElement = <Element> <unknown> this;
    } else {
      child.parentElement = null;
    }

    child._setOwnerDocument(this.#ownerDocument);
    this.#childNodesMutator.push(child);
  }

  removeChild(child: Node) {
    // TODO
  }

  replaceChild(newChild: Node, oldChild: Node): Node {
    if (oldChild.parentNode !== this) {
      throw new Error("Old child's parent is not the current node.");
    }
    oldChild.replaceWith(newChild);
    return oldChild;
  }

  private insertBeforeAfter(nodes: (Node | string)[], side: number) {
    const parentNode = this.parentNode!;
    const mutator = parentNode._getChildNodesMutator();
    const index = mutator.indexOf(this);
    nodes = nodesAndTextNodes(nodes, parentNode);

    mutator.splice(index + side, 0, ...(<Node[]> nodes));
  }

  before(...nodes: (Node | string)[]) {
    if (this.parentNode) {
      this.insertBeforeAfter(nodes, 0);
    }
  }

  after(...nodes: (Node | string)[]) {
    if (this.parentNode) {
      this.insertBeforeAfter(nodes, 1);
    }
  }

  replaceWith(...nodes: (Node | string)[]) {
    if (this.parentNode) {
      const parentNode = this.parentNode;
      const mutator = parentNode._getChildNodesMutator();
      const index = mutator.indexOf(this);
      nodes = nodesAndTextNodes(nodes, parentNode);

      mutator.splice(index, 1, ...(<Node[]> nodes));
      this.parentNode = this.parentElement = null;
    }
  }

  get children(): HTMLCollection {
    const collection = new HTMLCollection();
    const mutator = collection[HTMLCollectionMutatorSym]();

    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        mutator.push(<Element> child);
      }
    }

    return collection;
  }

  get nextSibling(): Node | null {
    const parent = this.parentNode;

    if (!parent) {
      return null;
    }

    const index = parent._getChildNodesMutator().indexOf(this);
    let next: Node | null = this.childNodes[index + 1] || null;

    return next;
  }

  get previousSibling(): Node | null {
    const parent = this.parentNode;

    if (!parent) {
      return null;
    }

    const index = parent._getChildNodesMutator().indexOf(this);
    let prev: Node | null = this.childNodes[index - 1] || null;

    return prev;
  }
}

export class CharacterData extends Node {
  constructor(
    public data: string,
    nodeName: string,
    nodeType: NodeType,
    parentNode: Node | null,
  ) {
    super(
      nodeName,
      nodeType,
      parentNode,
    );
    if (getLock()) {
      throw new TypeError("Illegal constructor");
    }

    this.nodeValue = data;
  }

  get length(): number {
    return this.data.length;
  }

  // TODO: Implement NonDocumentTypeChildNode.nextElementSibling, etc
  // ref: https://developer.mozilla.org/en-US/docs/Web/API/CharacterData
}

export class Text extends CharacterData {
  constructor(
    text: string = "",
  ) {
    let oldLock = getLock();
    setLock(false);
    super(
      text,
      "#text",
      NodeType.TEXT_NODE,
      null,
    );

    this.nodeValue = text;
    setLock(oldLock);
  }

  get textContent(): string {
    return <string> this.nodeValue;
  }
}

export class Comment extends CharacterData {
  constructor(
    text: string = "",
  ) {
    let oldLock = getLock();
    setLock(false);
    super(
      text,
      "#comment",
      NodeType.COMMENT_NODE,
      null,
    );

    this.nodeValue = text;
    setLock(oldLock);
  }

  get textContent(): string {
    return <string> this.nodeValue;
  }
}

