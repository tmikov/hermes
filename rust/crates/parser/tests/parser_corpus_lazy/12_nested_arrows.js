function outer() {
  var f = (x) => {
    var g = (y) => {
      return x + y;
    };
    return g;
  };
  return f;
}
