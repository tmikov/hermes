/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Conway's Game of Life — pure JavaScript implementation.
//
// Usage:
//   node life.js
//   hermes life.js

var WIDTH = 128;
var HEIGHT = 128;
var SIZE = WIDTH * HEIGHT;

var gridA = new Uint8Array(SIZE);
var gridB = new Uint8Array(SIZE);

function countNeighbors(grid, x, y) {
  var count = 0;
  for (var dy = -1; dy <= 1; dy++) {
    for (var dx = -1; dx <= 1; dx++) {
      if (dx === 0 && dy === 0)
        continue;
      var nx = (x + dx + WIDTH) % WIDTH;
      var ny = (y + dy + HEIGHT) % HEIGHT;
      count += grid[ny * WIDTH + nx];
    }
  }
  return count;
}

function step(src, dst) {
  for (var y = 0; y < HEIGHT; y++) {
    for (var x = 0; x < WIDTH; x++) {
      var n = countNeighbors(src, x, y);
      var alive = src[y * WIDTH + x];
      dst[y * WIDTH + x] = alive ? (n === 2 || n === 3 ? 1 : 0) : (n === 3 ? 1 : 0);
    }
  }
}

function countAlive(grid) {
  var count = 0;
  for (var i = 0; i < SIZE; i++)
    count += grid[i];
  return count;
}

function setCell(x, y) {
  gridA[y * WIDTH + x] = 1;
}

// Place R-pentomino at center:
//   .##
//   ##.
//   .#.
function initPattern() {
  gridA.fill(0);
  gridB.fill(0);
  var cx = (WIDTH / 2) | 0;
  var cy = (HEIGHT / 2) | 0;
  setCell(cx,     cy - 1);
  setCell(cx + 1, cy - 1);
  setCell(cx - 1, cy);
  setCell(cx,     cy);
  setCell(cx,     cy + 1);
}

function run(iterations) {
  initPattern();

  var src = gridA;
  var dst = gridB;

  for (var i = 0; i < iterations; i++) {
    step(src, dst);
    var tmp = src;
    src = dst;
    dst = tmp;
  }

  console.log(countAlive(src));
}

// Default: 2000 iterations on a 128x128 grid.
var t0 = Date.now();
run(2000);
console.log("elapsed:", Date.now() - t0, "ms");
