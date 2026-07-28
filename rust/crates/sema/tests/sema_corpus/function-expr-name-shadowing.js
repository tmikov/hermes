// The FunctionExprName scope wraps the function, so a parameter or a body
// declaration of the same name shadows the self-reference.
var byParam = function me(me) {
  return me;
};
var byVar = function me2() {
  var me2;
  return me2;
};
var visible = function me3() {
  return me3;
};
