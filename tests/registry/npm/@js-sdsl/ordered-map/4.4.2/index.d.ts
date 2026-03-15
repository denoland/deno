export declare class OrderedMapIterator<K, V> {
  equals(other: OrderedMapIterator<K, V>): boolean;
  next(): OrderedMapIterator<K, V>;
  readonly pointer: [K, V] | undefined;
}

export declare class OrderedMap<K, V> {
  constructor();
  find(key: K): OrderedMapIterator<K, V>;
  lowerBound(key: K): OrderedMapIterator<K, V>;
  end(): OrderedMapIterator<K, V>;
  setElement(key: K, value: V, hint?: OrderedMapIterator<K, V>): void;
  getElementByKey(key: K): V | undefined;
  eraseElementByKey(key: K): void;
  forEach(callback: (value: V, key: K, index: number) => void): void;
}
