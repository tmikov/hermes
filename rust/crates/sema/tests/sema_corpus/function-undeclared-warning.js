"use strict";
// resolveIdentifier's strict-mode UndefinedVariable warning names the
// enclosing function (FunctionContext::getFunctionName).
function named() {
  return missingInNamed;
}
var anonymous = function () {
  return missingInAnonymous;
};
var selfNamed = function inner() {
  return missingInInner;
};
