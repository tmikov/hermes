// visit(MetaPropertyNode *) (cpp:837-872): `new.target` inside a real
// function is fine, and so is `new.target` inside an arrow nested in one
// (nearestNonArrow walks past the arrow to the function).
function f() {
  return new.target;
}
function g() {
  var a = () => new.target;
  var b = () => () => new.target;
  return a;
}
var h = function () {
  return new.target;
};
