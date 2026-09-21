/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Compile with (requires Homebrew llvm + lld on macOS):
//   PATH="/opt/homebrew/opt/lld/bin:$PATH" \
//   /opt/homebrew/opt/llvm/bin/clang \
//     --target=wasm32-unknown-unknown -nostdlib -O2 \
//     -Wl,--no-entry -Wl,--export-all \
//     -o life.wasm life.c

#define WIDTH 128
#define HEIGHT 128
#define SIZE (WIDTH * HEIGHT)

__attribute__((import_module("env"), import_name("log")))
extern void env_log(int value);

static unsigned char gridA[SIZE];
static unsigned char gridB[SIZE];

static int count_neighbors(const unsigned char *grid, int x, int y) {
  int count = 0;
  for (int dy = -1; dy <= 1; dy++) {
    for (int dx = -1; dx <= 1; dx++) {
      if (dx == 0 && dy == 0)
        continue;
      int nx = (x + dx + WIDTH) % WIDTH;
      int ny = (y + dy + HEIGHT) % HEIGHT;
      count += grid[ny * WIDTH + nx];
    }
  }
  return count;
}

static void step(const unsigned char *src, unsigned char *dst) {
  for (int y = 0; y < HEIGHT; y++) {
    for (int x = 0; x < WIDTH; x++) {
      int n = count_neighbors(src, x, y);
      int alive = src[y * WIDTH + x];
      dst[y * WIDTH + x] = alive ? (n == 2 || n == 3) : (n == 3);
    }
  }
}

static int count_alive(const unsigned char *grid) {
  int count = 0;
  for (int i = 0; i < SIZE; i++)
    count += grid[i];
  return count;
}

static void clear(unsigned char *grid) {
  for (int i = 0; i < SIZE; i++)
    grid[i] = 0;
}

static void set_cell(int x, int y) {
  gridA[y * WIDTH + x] = 1;
}

// Place R-pentomino at center:
//   .##
//   ##.
//   .#.
static void init_pattern(void) {
  clear(gridA);
  clear(gridB);
  int cx = WIDTH / 2;
  int cy = HEIGHT / 2;
  set_cell(cx,     cy - 1);
  set_cell(cx + 1, cy - 1);
  set_cell(cx - 1, cy);
  set_cell(cx,     cy);
  set_cell(cx,     cy + 1);
}

__attribute__((export_name("run")))
void run(int iterations) {
  init_pattern();

  unsigned char *src = gridA;
  unsigned char *dst = gridB;

  for (int i = 0; i < iterations; i++) {
    step(src, dst);
    unsigned char *tmp = src;
    src = dst;
    dst = tmp;
  }

  env_log(count_alive(src));
}

__attribute__((export_name("main")))
void main_entry(void) {
  // Default: 2000 iterations on a 128x128 grid.
  run(2000);
}
