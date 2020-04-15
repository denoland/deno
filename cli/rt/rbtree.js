// Derived from https://github.com/vadimg/js_bintrees. MIT Licensed.
System.register("$deno$/rbtree.ts", ["$deno$/util.ts"], function (
  exports_19,
  context_19
) {
  "use strict";
  let util_ts_3, RBNode, RBTree;
  const __moduleName = context_19 && context_19.id;
  function isRed(node) {
    return node !== null && node.red;
  }
  function singleRotate(root, dir) {
    const save = root.getChild(!dir);
    util_ts_3.assert(save);
    root.setChild(!dir, save.getChild(dir));
    save.setChild(dir, root);
    root.red = true;
    save.red = false;
    return save;
  }
  function doubleRotate(root, dir) {
    root.setChild(!dir, singleRotate(root.getChild(!dir), !dir));
    return singleRotate(root, dir);
  }
  return {
    setters: [
      function (util_ts_3_1) {
        util_ts_3 = util_ts_3_1;
      },
    ],
    execute: function () {
      RBNode = class RBNode {
        constructor(data) {
          this.data = data;
          this.left = null;
          this.right = null;
          this.red = true;
        }
        getChild(dir) {
          return dir ? this.right : this.left;
        }
        setChild(dir, val) {
          if (dir) {
            this.right = val;
          } else {
            this.left = val;
          }
        }
      };
      RBTree = class RBTree {
        constructor(comparator) {
          this.#comparator = comparator;
          this.#root = null;
        }
        #comparator;
        #root;
        /** Returns `null` if tree is empty. */
        min() {
          let res = this.#root;
          if (res === null) {
            return null;
          }
          while (res.left !== null) {
            res = res.left;
          }
          return res.data;
        }
        /** Returns node `data` if found, `null` otherwise. */
        find(data) {
          let res = this.#root;
          while (res !== null) {
            const c = this.#comparator(data, res.data);
            if (c === 0) {
              return res.data;
            } else {
              res = res.getChild(c > 0);
            }
          }
          return null;
        }
        /** returns `true` if inserted, `false` if duplicate. */
        insert(data) {
          let ret = false;
          if (this.#root === null) {
            // empty tree
            this.#root = new RBNode(data);
            ret = true;
          } else {
            const head = new RBNode(null); // fake tree root
            let dir = 0;
            let last = 0;
            // setup
            let gp = null; // grandparent
            let ggp = head; // grand-grand-parent
            let p = null; // parent
            let node = this.#root;
            ggp.right = this.#root;
            // search down
            while (true) {
              if (node === null) {
                // insert new node at the bottom
                node = new RBNode(data);
                p.setChild(dir, node);
                ret = true;
              } else if (isRed(node.left) && isRed(node.right)) {
                // color flip
                node.red = true;
                node.left.red = false;
                node.right.red = false;
              }
              // fix red violation
              if (isRed(node) && isRed(p)) {
                const dir2 = ggp.right === gp;
                util_ts_3.assert(gp);
                if (node === p.getChild(last)) {
                  ggp.setChild(dir2, singleRotate(gp, !last));
                } else {
                  ggp.setChild(dir2, doubleRotate(gp, !last));
                }
              }
              const cmp = this.#comparator(node.data, data);
              // stop if found
              if (cmp === 0) {
                break;
              }
              last = dir;
              dir = Number(cmp < 0); // Fix type
              // update helpers
              if (gp !== null) {
                ggp = gp;
              }
              gp = p;
              p = node;
              node = node.getChild(dir);
            }
            // update root
            this.#root = head.right;
          }
          // make root black
          this.#root.red = false;
          return ret;
        }
        /** Returns `true` if removed, `false` if not found. */
        remove(data) {
          if (this.#root === null) {
            return false;
          }
          const head = new RBNode(null); // fake tree root
          let node = head;
          node.right = this.#root;
          let p = null; // parent
          let gp = null; // grand parent
          let found = null; // found item
          let dir = 1;
          while (node.getChild(dir) !== null) {
            const last = dir;
            // update helpers
            gp = p;
            p = node;
            node = node.getChild(dir);
            const cmp = this.#comparator(data, node.data);
            dir = cmp > 0;
            // save found node
            if (cmp === 0) {
              found = node;
            }
            // push the red node down
            if (!isRed(node) && !isRed(node.getChild(dir))) {
              if (isRed(node.getChild(!dir))) {
                const sr = singleRotate(node, dir);
                p.setChild(last, sr);
                p = sr;
              } else if (!isRed(node.getChild(!dir))) {
                const sibling = p.getChild(!last);
                if (sibling !== null) {
                  if (
                    !isRed(sibling.getChild(!last)) &&
                    !isRed(sibling.getChild(last))
                  ) {
                    // color flip
                    p.red = false;
                    sibling.red = true;
                    node.red = true;
                  } else {
                    util_ts_3.assert(gp);
                    const dir2 = gp.right === p;
                    if (isRed(sibling.getChild(last))) {
                      gp.setChild(dir2, doubleRotate(p, last));
                    } else if (isRed(sibling.getChild(!last))) {
                      gp.setChild(dir2, singleRotate(p, last));
                    }
                    // ensure correct coloring
                    const gpc = gp.getChild(dir2);
                    util_ts_3.assert(gpc);
                    gpc.red = true;
                    node.red = true;
                    util_ts_3.assert(gpc.left);
                    gpc.left.red = false;
                    util_ts_3.assert(gpc.right);
                    gpc.right.red = false;
                  }
                }
              }
            }
          }
          // replace and remove if found
          if (found !== null) {
            found.data = node.data;
            util_ts_3.assert(p);
            p.setChild(p.right === node, node.getChild(node.left === null));
          }
          // update root and make it black
          this.#root = head.right;
          if (this.#root !== null) {
            this.#root.red = false;
          }
          return found !== null;
        }
      };
      exports_19("RBTree", RBTree);
    },
  };
});
