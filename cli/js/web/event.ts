// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as domTypes from "./dom_types.d.ts";
import { defineEnumerableProps, requiredArguments } from "./util.ts";
import { assert } from "../util.ts";

/** Stores a non-accessible view of the event path which is used internally in
 * the logic for determining the path of an event. */
export interface EventPath {
  item: EventTarget;
  itemInShadowTree: boolean;
  relatedTarget: EventTarget | null;
  rootOfClosedTree: boolean;
  slotInClosedTree: boolean;
  target: EventTarget | null;
  touchTargetList: EventTarget[];
}

interface EventAttributes {
  type: string;
  bubbles: boolean;
  cancelable: boolean;
  composed: boolean;
  currentTarget: EventTarget | null;
  eventPhase: number;
  target: EventTarget | null;
  timeStamp: number;
}

interface EventData {
  dispatched: boolean;
  inPassiveListener: boolean;
  isTrusted: boolean;
  path: EventPath[];
  stopImmediatePropagation: boolean;
}

const eventData = new WeakMap<Event, EventData>();

// accessors for non runtime visible data

export function getDispatched(event: Event): boolean {
  return Boolean(eventData.get(event)?.dispatched);
}

export function getPath(event: Event): EventPath[] {
  return eventData.get(event)?.path ?? [];
}

export function getStopImmediatePropagation(event: Event): boolean {
  return Boolean(eventData.get(event)?.stopImmediatePropagation);
}

export function setCurrentTarget(
  event: Event,
  value: EventTarget | null
): void {
  (event as EventImpl).currentTarget = value;
}

export function setDispatched(event: Event, value: boolean): void {
  const data = eventData.get(event as Event);
  if (data) {
    data.dispatched = value;
  }
}

export function setEventPhase(event: Event, value: number): void {
  (event as EventImpl).eventPhase = value;
}

export function setInPassiveListener(event: Event, value: boolean): void {
  const data = eventData.get(event as Event);
  if (data) {
    data.inPassiveListener = value;
  }
}

export function setPath(event: Event, value: EventPath[]): void {
  const data = eventData.get(event as Event);
  if (data) {
    data.path = value;
  }
}

export function setRelatedTarget<T extends Event>(
  event: T,
  value: EventTarget | null
): void {
  if ("relatedTarget" in event) {
    (event as T & {
      relatedTarget: EventTarget | null;
    }).relatedTarget = value;
  }
}

export function setTarget(event: Event, value: EventTarget | null): void {
  (event as EventImpl).target = value;
}

export function setStopImmediatePropagation(
  event: Event,
  value: boolean
): void {
  const data = eventData.get(event as Event);
  if (data) {
    data.stopImmediatePropagation = value;
  }
}

// Type guards that widen the event type

export function hasRelatedTarget(
  event: Event
): event is domTypes.FocusEvent | domTypes.MouseEvent {
  return "relatedTarget" in event;
}

function isTrusted(this: Event): boolean {
  return eventData.get(this)!.isTrusted;
}

export class EventImpl implements Event {
  // The default value is `false`.
  // Use `defineProperty` to define on each instance, NOT on the prototype.
  isTrusted!: boolean;

  #canceledFlag = false;
  #stopPropagationFlag = false;
  #attributes: EventAttributes;

  constructor(type: string, eventInitDict: EventInit = {}) {
    requiredArguments("Event", arguments.length, 1);
    type = String(type);
    this.#attributes = {
      type,
      bubbles: eventInitDict.bubbles ?? false,
      cancelable: eventInitDict.cancelable ?? false,
      composed: eventInitDict.composed ?? false,
      currentTarget: null,
      eventPhase: Event.NONE,
      target: null,
      timeStamp: Date.now(),
    };
    eventData.set(this, {
      dispatched: false,
      inPassiveListener: false,
      isTrusted: false,
      path: [],
      stopImmediatePropagation: false,
    });
    Reflect.defineProperty(this, "isTrusted", {
      enumerable: true,
      get: isTrusted,
    });
  }

  get bubbles(): boolean {
    return this.#attributes.bubbles;
  }

  get cancelBubble(): boolean {
    return this.#stopPropagationFlag;
  }

  set cancelBubble(value: boolean) {
    this.#stopPropagationFlag = value;
  }

  get cancelable(): boolean {
    return this.#attributes.cancelable;
  }

  get composed(): boolean {
    return this.#attributes.composed;
  }

  get currentTarget(): EventTarget | null {
    return this.#attributes.currentTarget;
  }

  set currentTarget(value: EventTarget | null) {
    this.#attributes = {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: value,
      eventPhase: this.eventPhase,
      target: this.target,
      timeStamp: this.timeStamp,
    };
  }

  get defaultPrevented(): boolean {
    return this.#canceledFlag;
  }

  get eventPhase(): number {
    return this.#attributes.eventPhase;
  }

  set eventPhase(value: number) {
    this.#attributes = {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: this.currentTarget,
      eventPhase: value,
      target: this.target,
      timeStamp: this.timeStamp,
    };
  }

  get initialized(): boolean {
    return true;
  }

  get target(): EventTarget | null {
    return this.#attributes.target;
  }

  set target(value: EventTarget | null) {
    this.#attributes = {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: this.currentTarget,
      eventPhase: this.eventPhase,
      target: value,
      timeStamp: this.timeStamp,
    };
  }

  get timeStamp(): number {
    return this.#attributes.timeStamp;
  }

  get type(): string {
    return this.#attributes.type;
  }

  composedPath(): EventTarget[] {
    const path = eventData.get(this)!.path;
    if (path.length === 0) {
      return [];
    }

    assert(this.currentTarget);
    const composedPath: EventPath[] = [
      {
        item: this.currentTarget,
        itemInShadowTree: false,
        relatedTarget: null,
        rootOfClosedTree: false,
        slotInClosedTree: false,
        target: null,
        touchTargetList: [],
      },
    ];

    let currentTargetIndex = 0;
    let currentTargetHiddenSubtreeLevel = 0;

    for (let index = path.length - 1; index >= 0; index--) {
      const { item, rootOfClosedTree, slotInClosedTree } = path[index];

      if (rootOfClosedTree) {
        currentTargetHiddenSubtreeLevel++;
      }

      if (item === this.currentTarget) {
        currentTargetIndex = index;
        break;
      }

      if (slotInClosedTree) {
        currentTargetHiddenSubtreeLevel--;
      }
    }

    let currentHiddenLevel = currentTargetHiddenSubtreeLevel;
    let maxHiddenLevel = currentTargetHiddenSubtreeLevel;

    for (let i = currentTargetIndex - 1; i >= 0; i--) {
      const { item, rootOfClosedTree, slotInClosedTree } = path[i];

      if (rootOfClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        composedPath.unshift({
          item,
          itemInShadowTree: false,
          relatedTarget: null,
          rootOfClosedTree: false,
          slotInClosedTree: false,
          target: null,
          touchTargetList: [],
        });
      }

      if (slotInClosedTree) {
        currentHiddenLevel--;

        if (currentHiddenLevel < maxHiddenLevel) {
          maxHiddenLevel = currentHiddenLevel;
        }
      }
    }

    currentHiddenLevel = currentTargetHiddenSubtreeLevel;
    maxHiddenLevel = currentTargetHiddenSubtreeLevel;

    for (let index = currentTargetIndex + 1; index < path.length; index++) {
      const { item, rootOfClosedTree, slotInClosedTree } = path[index];

      if (slotInClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        composedPath.push({
          item,
          itemInShadowTree: false,
          relatedTarget: null,
          rootOfClosedTree: false,
          slotInClosedTree: false,
          target: null,
          touchTargetList: [],
        });
      }

      if (rootOfClosedTree) {
        currentHiddenLevel--;

        if (currentHiddenLevel < maxHiddenLevel) {
          maxHiddenLevel = currentHiddenLevel;
        }
      }
    }
    return composedPath.map((p) => p.item);
  }

  preventDefault(): void {
    if (this.cancelable && !eventData.get(this)!.inPassiveListener) {
      this.#canceledFlag = true;
    }
  }

  stopPropagation(): void {
    this.#stopPropagationFlag = true;
  }

  stopImmediatePropagation(): void {
    this.#stopPropagationFlag = true;
    eventData.get(this)!.stopImmediatePropagation = true;
  }

  get NONE(): number {
    return Event.NONE;
  }

  get CAPTURING_PHASE(): number {
    return Event.CAPTURING_PHASE;
  }

  get AT_TARGET(): number {
    return Event.AT_TARGET;
  }

  get BUBBLING_PHASE(): number {
    return Event.BUBBLING_PHASE;
  }

  static get NONE(): number {
    return 0;
  }

  static get CAPTURING_PHASE(): number {
    return 1;
  }

  static get AT_TARGET(): number {
    return 2;
  }

  static get BUBBLING_PHASE(): number {
    return 3;
  }
}

defineEnumerableProps(EventImpl, [
  "bubbles",
  "cancelable",
  "composed",
  "currentTarget",
  "defaultPrevented",
  "eventPhase",
  "target",
  "timeStamp",
  "type",
]);
