// ...and a function with its own "use strict" directive becomes strict even
// though the program (and its sibling) is loose.
function loose(a) {
  return a;
}
function strict(b) {
  "use strict";
  return b;
}
