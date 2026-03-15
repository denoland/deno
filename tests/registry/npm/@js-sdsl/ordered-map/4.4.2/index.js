"use strict";

class OrderedMapIterator {
  constructor(map, index) {
    this._map = map;
    this._index = index;
  }

  equals(other) {
    return this._map === other._map && this._index === other._index;
  }

  next() {
    return new OrderedMapIterator(this._map, this._index + 1);
  }

  get pointer() {
    const entry = this._map._entries[this._index];
    if (!entry) {
      return undefined;
    }
    return [entry[0], entry[1]];
  }
}

class OrderedMap {
  constructor() {
    this._entries = [];
  }

  _findIndex(key) {
    let low = 0;
    let high = this._entries.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if (this._entries[mid][0] < key) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    return low;
  }

  find(key) {
    const index = this._findIndex(key);
    if (index < this._entries.length && this._entries[index][0] === key) {
      return new OrderedMapIterator(this, index);
    }
    return this.end();
  }

  lowerBound(key) {
    return new OrderedMapIterator(this, this._findIndex(key));
  }

  end() {
    return new OrderedMapIterator(this, this._entries.length);
  }

  setElement(key, value, hint) {
    let index = hint && hint._map === this ? hint._index : this._findIndex(key);
    if (index < this._entries.length && this._entries[index][0] === key) {
      this._entries[index][1] = value;
      return;
    }
    this._entries.splice(index, 0, [key, value]);
  }

  getElementByKey(key) {
    const index = this._findIndex(key);
    if (index < this._entries.length && this._entries[index][0] === key) {
      return this._entries[index][1];
    }
    return undefined;
  }

  eraseElementByKey(key) {
    const index = this._findIndex(key);
    if (index < this._entries.length && this._entries[index][0] === key) {
      this._entries.splice(index, 1);
    }
  }

  forEach(callback) {
    this._entries.forEach((entry, index) => callback(entry[1], entry[0], index));
  }
}

module.exports = {
  OrderedMap,
  OrderedMapIterator,
};
