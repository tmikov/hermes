"use strict";
let {a, b: [c], ...rest} = x;
a;
b;
c;
rest;
let [d = defaultD, , e = defaultE] = y;
