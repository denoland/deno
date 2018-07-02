export { X }

const X = 24;

class Test {}

export { Test as T }

const P = {};

/**
 * Some valuable doc.
 * TODO Parser already ignores this, should be fixed.
 */
P.name = {};

export default P;
