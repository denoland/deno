function branch(outer: boolean, inner:boolean) {
  if (outer) {
    console.log("outer: true");

    if (inner) {
      console.log("inner: true");
    } else {
      console.log("inner: false");
    }
  } else {
    console.log("outer: false");

    if (inner) {
      console.log("inner: true");
    } else {
      console.log("inner: false");
    }
  }
}

branch(true, false);
branch(false, true);
