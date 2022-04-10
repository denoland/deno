import { getLock } from "../constructor-lock.ts";
import { fragmentNodesFromString } from "../deserialize.ts";
import { Node, NodeType, Text } from "./node.ts";
import { NodeList, nodeListMutatorSym } from "./node-list.ts";

export class DOMTokenList extends Set<string> {
  contains(token: string): boolean {
    return this.has(token);
  }
}

let attrLock = true;
export class Attr {
  #namedNodeMap: NamedNodeMap | null = null;
  #name: string = "";

  constructor(map: NamedNodeMap, name: string) {
    if (attrLock) {
      throw new TypeError("Illegal constructor");
    }

    this.#name = name;
    this.#namedNodeMap = map;
  }

  get name() {
    return this.#name;
  }

  get value() {
    return (<{[attribute: string]: string}> <unknown> this.#namedNodeMap)[this.#name];
  }
}

export class NamedNodeMap {
  #attrObjCache: {
    [attribute: string]: Attr;
  } = {};

  private newAttr(attribute: string): Attr {
    attrLock = false;
    const attr = new Attr(this, attribute);
    attrLock = true;
    return attr;
  }

  getNamedItem(attribute: string) {
    return this.#attrObjCache[attribute] ?? (this.#attrObjCache[attribute] = this.newAttr(attribute));
  }

  setNamedItem(...args: any) {
    // TODO
  }
}

export class Element extends Node {
  public classList = new DOMTokenList();
  public attributes: NamedNodeMap & {[attribute: string]: string} = <any> new NamedNodeMap();

  #currentId = "";

  constructor(
    public tagName: string,
    parentNode: Node | null,
    attributes: [string, string][],
  ) {
    super(
      tagName,
      NodeType.ELEMENT_NODE,
      parentNode,
    );
    if (getLock()) {
      throw new TypeError("Illegal constructor");
    }

    for (const attr of attributes) {
      this.attributes[attr[0]] = attr[1];

      switch (attr[0]) {
        case "class":
          this.classList = new DOMTokenList(attr[1].split(/\s+/g));
          break;
        case "id":
          this.#currentId = attr[1];
          break;
      }
    }

    this.tagName = this.nodeName = tagName.toUpperCase();
  }

  get className(): string {
    return Array.from(this.classList).join(" ");
  }

  set className(className: string) {
    // TODO: Probably don't replace the current classList
    this.classList = new DOMTokenList(className.split(/\s+/g));
  }

  get outerHTML(): string {
    const tagName = this.tagName.toLowerCase();
    const attributes = this.attributes;
    let out = "<" + tagName;

    for (const attribute of Object.getOwnPropertyNames(attributes)) {
      out += ` ${ attribute.toLowerCase() }`;

      if (attributes[attribute] != null) {
        out += `="${
          attributes[attribute]
              .replace(/&/g, "&amp;")
              .replace(/</g, "&lt;")
              .replace(/>/g, "&gt;")
              .replace(/"/g, "&quot;")
        }"`;
      }
    }

    // Special handling for void elements
    switch (tagName) {
      case "area":
      case "base":
      case "br":
      case "col":
      case "embed":
      case "hr":
      case "img":
      case "input":
      case "link":
      case "meta":
      case "param":
      case "source":
      case "track":
      case "wbr":
        out += ">";
        break;

      default:
        out += ">" + this.innerHTML + `</${ tagName }>`;
        break;
    }

    return out;
  }

  set outerHTML(html: string) {
    // TODO: Someday...
  }

  get innerHTML(): string {
    let out = "";

    for (const child of this.childNodes) {
      switch (child.nodeType) {
        case NodeType.ELEMENT_NODE:
          out += (<Element> child).outerHTML;
          break;
        case NodeType.TEXT_NODE:
          out += (<Text> child).data
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;");
          break;
      }
    }

    return out;
  }

  set innerHTML(html: string) {
    // Remove all children
    for (const child of this.childNodes) {
      child.parentNode = child.parentElement = null;
    }

    const mutator = this._getChildNodesMutator();
    mutator.splice(0, this.childNodes.length);

    if (html.length) {
      const parsed = fragmentNodesFromString(html);
      mutator.push(...parsed.childNodes[0].childNodes);

      for (const child of this.childNodes) {
        child.parentNode = child.parentElement = this;
        child._setOwnerDocument(this.ownerDocument);
      }
    }
  }

  get id(): string {
    return this.#currentId || "";
  }

  set id(id: string) {
    this.setAttribute(id, this.#currentId = id);
  }

  getAttribute(name: string): string | null {
    return this.attributes[name] ?? null;
  }

  setAttribute(name: string, value: any) {
    this.attributes[name] = "" + value;

    if (name === "id") {
      this.#currentId = value;
    }
  }

  hasAttribute(name: string): boolean {
    return this.attributes.hasOwnProperty(name);
  }

  hasAttributeNS(_namespace: string, name: string): boolean {
    // TODO: Use namespace
    return this.attributes.hasOwnProperty(name);
  }

  get nextElementSibling(): Element | null {
    const parent = this.parentNode;

    if (!parent) {
      return null;
    }

    const index = parent._getChildNodesMutator().indexOf(this);
    const childNodes = parent.childNodes;
    let next: Element | null = null;

    for (let i = index + 1; i < childNodes.length; i++) {
      const sibling = childNodes[i];

      if (sibling.nodeType === NodeType.ELEMENT_NODE) {
        next = <Element> sibling;
        break;
      }
    }

    return next;
  }

  get previousElementSibling(): Element | null {
    const parent = this.parentNode;

    if (!parent) {
      return null;
    }

    const index = parent._getChildNodesMutator().indexOf(this);
    const childNodes = parent.childNodes;
    let prev: Element | null = null;

    for (let i = index - 1; i >= 0; i--) {
      const sibling = childNodes[i];

      if (sibling.nodeType === NodeType.ELEMENT_NODE) {
        prev = <Element> sibling;
        break;
      }
    }

    return prev;
  }

  querySelector(selectors: string): Element | null {
    if (!this.ownerDocument) {
      throw new Error("Element must have an owner document");
    }

    return this.ownerDocument!._nwapi.first(selectors, this);
  }

  querySelectorAll(selectors: string): NodeList {
    if (!this.ownerDocument) {
      throw new Error("Element must have an owner document");
    }

    const nodeList = new NodeList();
    const mutator = nodeList[nodeListMutatorSym]();
    mutator.push(...this.ownerDocument!._nwapi.select(selectors, this))

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
    return <Element[]> this._getElementsByTagName(tagName.toUpperCase(), []);
  }

  private _getElementsByTagNameWildcard(search: Node[]): Node[] {
    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        search.push(child);
        (<Element> child)._getElementsByTagNameWildcard(search);
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

        (<Element> child)._getElementsByTagName(tagName, search);
      }
    }

    return search;
  }

  getElementsByClassName(className: string): Element[] {
    return <Element[]> this._getElementsByClassName(className, []);
  }

  getElementsByTagNameNS(_namespace: string, localName: string): Element[] {
    // TODO: Use namespace
    return this.getElementsByTagName(localName);
  }

  private _getElementsByClassName(className: string, search: Node[]): Node[] {
    for (const child of this.childNodes) {
      if (child.nodeType === NodeType.ELEMENT_NODE) {
        if ((<Element> child).classList.contains(className)) {
          search.push(child);
        }

        (<Element> child)._getElementsByClassName(className, search);
      }
    }

    return search;
  }
}

