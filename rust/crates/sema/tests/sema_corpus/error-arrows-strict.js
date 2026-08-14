// In strict mode `arguments` and `eval` cannot be declared, including as
// arrow parameters (validateDeclarationName, reached through declareParams).
"use strict";
var f = (x) => x;
var g = (arguments) => arguments;
var h = (eval) => eval;
