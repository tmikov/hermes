// uniqueParams is also forced by a NON-simple parameter list, even in loose
// mode, so this duplicate is an error without any "use strict".
function f([a], a) {
  return a;
}
