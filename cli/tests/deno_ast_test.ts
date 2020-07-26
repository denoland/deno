// This example is mostly in TypeScript, because it is much
// easier to understand this way first. At the end we'll
// cover how to create the same class but using JSDoc instead.

// Generic Classes are a way to say that a particular type
// depends on another type. For example, here is a drawer
// which can hold any sort of object, but only one type:

class Drawer<ClothingType> {
  contents: ClothingType[] = [];

  add(object: ClothingType) {
    this.contents.push(object);
  }

  remove() {
    return this.contents.pop();
  }
}

// In order to use a Drawer, you will need another
// type to work with:

interface Sock {
  color: string;
}

interface TShirt {
  size: "s" | "m" | "l";
}

// We can create a Drawer just for socks by passing in the
// type Sock when we create a new Drawer:
const sockDrawer = new Drawer<Sock>();

// Now we can add or remove socks to the drawer:
sockDrawer.add({ color: "white" });
const mySock = sockDrawer.remove();

// As well as creating a drawer for TShirts:
const tshirtDrawer = new Drawer<TShirt>();
tshirtDrawer.add({ size: "m" });

// If you're a bit eccentric, you could even create a drawer
// which mixes Socks and TShirts by using a union:

const mixedDrawer = new Drawer<Sock | TShirt>();

// Creating a class like Drawer without the extra TypeScript
// syntax requires using the template tag in JSDoc. In this
// example we define the template variable, then provide
// the properties on the class:

// To have this work in the playground, you'll need to change
// the settings to be a JavaScript file, and delete the
// TypeScript code above

/**
 * @template {{}} ClothingType
 */
class Dresser {
  constructor() {
    /** @type {ClothingType[]} */
    this.contents = [];
  }

  /** @param {ClothingType} object */
  add(object) {
    this.contents.push(object);
  }

  /** @return {ClothingType} */
  remove() {
    return this.contents.pop();
  }
}

// Then we create a new type via JSDoc:

/**
 * @typedef {Object} Coat An item of clothing
 * @property {string} color The colour for coat
 */

// Then when we create a new instance of that class
// we use @type to assign the variable as a Dresser
// which handles Coats.

/** @type {Dresser<Coat>} */
const coatDresser = new Dresser();

coatDresser.add({ color: "green" });
const coat = coatDresser.remove();
