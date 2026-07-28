// The two `new.target` error shapes (cpp:842-856). At global scope BOTH fire:
// isGlobalScope() is true, and nearestNonArrow(global) IS the global
// function. Inside a global arrow only the second one fires.
new.target;
var a = () => new.target;
var b = () => () => new.target;
