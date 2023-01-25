import fsevents from "npm:fsevents";
console.log(fsevents);

fsevents.watch("./", (path, flags, id) => {
  console.log(path, flags, id);
});
