var storage = localStorage;
storage.clear();

const key = 9;
const value = "value for " + storage.name;
const expected = value;

storage[key] = value;

console.log(storage.getItem(key));
