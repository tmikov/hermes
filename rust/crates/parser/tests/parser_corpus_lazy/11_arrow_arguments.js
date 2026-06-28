function outer() {
  var f = () => arguments[0];
  return f;
}
