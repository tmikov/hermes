/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const fs = require('fs');
const path = require('path');

const OUTPUT_FILE = path.resolve(
  __dirname,
  '../hermes-parser-native/src/HermesParserKindHash.js',
);

/**
 * FNV-1a over each name followed by a newline. Must stay identical to
 * computeKindHash() in tools/hermes-parser-native/KindHash.h.
 */
function fnv1a(names) {
  let hash = 0x811c9dc5;
  const feed = byte => {
    hash ^= byte;
    hash = Math.imul(hash, 16777619) >>> 0;
  };
  for (const name of names) {
    for (let i = 0; i < name.length; i++) {
      feed(name.charCodeAt(i) & 0xff);
    }
    feed(0x0a);
  }
  return hash >>> 0;
}

/**
 * Extract the ordered node-kind names from ESTree.def. ESTREE_FIRST(X) and
 * ESTREE_LAST(X) contribute "XFirst" and "XLast"; every other macro
 * contributes the bare name. This ordering matches NODE_DESERIALIZERS.
 */
function extractNames(defPath) {
  const text = fs.readFileSync(defPath, 'utf8').replace(/\n/g, ' ');
  const re = /ESTREE_(NODE_\d+_ARGS|FIRST|LAST)\(\s*([A-Za-z0-9_]+)/g;
  const names = [];
  let m;
  while ((m = re.exec(text)) !== null) {
    // Skip the macro definitions at the top of the file, which use the
    // literal parameter name NAME rather than a real node name.
    if (m[2] === 'NAME') {
      continue;
    }
    if (m[1] === 'FIRST') {
      names.push(m[2] + 'First');
    } else if (m[1] === 'LAST') {
      names.push(m[2] + 'Last');
    } else {
      names.push(m[2]);
    }
  }
  return names;
}

const includePath = process.argv[2];
if (includePath == null) {
  console.error('usage: genKindHash.js <hermes-include-path>');
  process.exit(1);
}

const names = extractNames(path.join(includePath, 'hermes/AST/ESTree.def'));
const hash = fnv1a(names);

fs.writeFileSync(
  OUTPUT_FILE,
  `/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict
 * @format
 * @generated
 */

'use strict';

// Hash of the ${names.length} node-kind names in ESTree.def, in order.
// Must match computeKindHash() in tools/hermes-parser-native/KindHash.h.
export default ${hash};
`,
);

console.log(`genKindHash: ${names.length} kinds, hash ${hash}`);
