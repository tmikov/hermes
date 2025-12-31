/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_AST_JSXTRANSFORM_H
#define HERMES_AST_JSXTRANSFORM_H

#include "llvh/ADT/StringRef.h"

#include <string>

namespace hermes {

class Context;

namespace ESTree {
class Node;
} // namespace ESTree

/// JSX runtime mode.
enum class JSXRuntime {
  /// JSX: uses jsx/jsxs/jsxDEV/Fragment from a configurable global.
  /// This is the React 17+ style transform.
  JSX,
  /// CreateElement: uses React.createElement/Fragment.
  /// This is the legacy transform.
  CreateElement,
};

/// Configuration for JSX transformation.
struct JSXTransformConfig {
  /// The runtime mode (jsx vs createElement).
  JSXRuntime runtime = JSXRuntime::JSX;

  /// Development mode enables jsxDEV with source location info.
  bool development = false;

  /// For jsx mode: the global object to access jsx/jsxs/jsxDEV/Fragment.
  /// Default: "JSX" -> JSX.jsx, JSX.jsxs, JSX.jsxDEV, JSX.Fragment
  std::string jsxGlobal = "JSX";

  /// For createElement mode: the global object for createElement/Fragment.
  /// Default: "React" -> React.createElement, React.Fragment
  std::string createElementGlobal = "React";
};

/// Transform JSX AST nodes into function calls.
///
/// This transformation converts JSX elements and fragments into standard
/// JavaScript function calls. The transformation mode (jsx vs createElement)
/// and other options are controlled by the config parameter.
///
/// JSX mode (React 17+):
///   <div className="foo">{x}</div>
///   -> JSX.jsx("div", { className: "foo", children: x })
///
///   <div><span>A</span><span>B</span></div>
///   -> JSX.jsxs("div", { children: [...] })
///
/// CreateElement mode:
///   <div className="foo">{x}</div>
///   -> React.createElement("div", { className: "foo" }, x)
///
/// \param context The AST context.
/// \param node The root AST node to transform.
/// \param config The JSX transformation configuration.
/// \param sourceFilename The source file name (for development mode).
/// \return The transformed AST node.
ESTree::Node *transformJSX(
    Context &context,
    ESTree::Node *node,
    const JSXTransformConfig &config,
    llvh::StringRef sourceFilename = "");

} // namespace hermes

#endif // HERMES_AST_JSXTRANSFORM_H
