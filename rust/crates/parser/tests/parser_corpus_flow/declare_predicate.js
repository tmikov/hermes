declare function isString(x: mixed): boolean %checks;
declare function isStr(x: mixed): boolean %checks(typeof x === "string");
declare function plain(x: mixed): boolean;
function checksInline(x: mixed): boolean %checks {
  return typeof x === "string";
}
