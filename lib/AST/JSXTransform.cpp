/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/AST/JSXTransform.h"
#include "hermes/AST/TransformationsBase.h"

#include "llvh/ADT/SmallString.h"

namespace hermes {

namespace {

/// Transforms JSX AST nodes into function calls.
class JSXTransformer : public TransformationsBase {
 public:
  static constexpr bool kEnableNodeListMutation = true;

  JSXTransformer(
      Context &context,
      const JSXTransformConfig &config,
      llvh::StringRef sourceFilename)
      : TransformationsBase(context),
        config_(config),
        sourceFilename_(sourceFilename) {}

  void visit(ESTree::JSXElementNode *node, ESTree::Node **ppNode) {
    // First, recursively transform any nested JSX in children and attributes.
    visitESTreeChildren(*this, node);

    // Now transform this JSX element into a function call.
    *ppNode = transformJSXElement(node);
  }

  void visit(ESTree::JSXFragmentNode *node, ESTree::Node **ppNode) {
    // First, recursively transform any nested JSX in children.
    visitESTreeChildren(*this, node);

    // Now transform this JSX fragment into a function call.
    *ppNode = transformJSXFragment(node);
  }

  void visit(ESTree::Node *node) {
    visitESTreeChildren(*this, node);
  }

 private:
  const JSXTransformConfig &config_;
  llvh::StringRef sourceFilename_;

  /// Transform a JSXElement into a function call.
  ESTree::Node *transformJSXElement(ESTree::JSXElementNode *element) {
    auto *opening = llvh::cast<ESTree::JSXOpeningElementNode>(
        element->_openingElement);

    // Build the element type (string literal or expression).
    ESTree::Node *elementType = buildElementType(opening->_name);

    // Collect children, handling JSXText, expressions, spread children, etc.
    NodeVector children;
    bool hasSpreadChild = false;
    collectChildren(element->_children, children, hasSpreadChild);

    // Extract key from attributes and build props object.
    ESTree::Node *keyExpr = nullptr;
    ESTree::Node *propsExpr = buildProps(
        element, opening->_attributes, children, hasSpreadChild, &keyExpr);

    // Create the function call based on runtime mode.
    if (config_.runtime == JSXRuntime::JSX) {
      return createAutomaticCall(
          element, elementType, propsExpr, keyExpr, children.size() > 1);
    } else {
      return createClassicCall(element, elementType, propsExpr, children);
    }
  }

  /// Transform a JSXFragment into a function call.
  ESTree::Node *transformJSXFragment(ESTree::JSXFragmentNode *fragment) {
    // Collect children.
    NodeVector children;
    bool hasSpreadChild = false;
    collectChildren(fragment->_children, children, hasSpreadChild);

    // Build Fragment type expression.
    ESTree::Node *fragmentType = buildFragmentType(fragment);

    // Build props with children.
    ESTree::Node *propsExpr =
        buildPropsWithChildren(fragment, children, hasSpreadChild);

    // Create the function call.
    if (config_.runtime == JSXRuntime::JSX) {
      return createAutomaticCall(
          fragment, fragmentType, propsExpr, nullptr, children.size() > 1);
    } else {
      return createClassicCall(fragment, fragmentType, nullptr, children);
    }
  }

  /// Build the element type expression from the JSX element name.
  /// - Lowercase identifiers become string literals: <div> -> "div"
  /// - Uppercase identifiers become identifiers: <Comp> -> Comp
  /// - Member expressions: <Foo.Bar> -> Foo.Bar
  /// - Namespaced names: <ns:tag> -> "ns:tag"
  ESTree::Node *buildElementType(ESTree::Node *name) {
    if (auto *ident = llvh::dyn_cast<ESTree::JSXIdentifierNode>(name)) {
      llvh::StringRef identName = ident->_name->str();
      // Check if first character is lowercase -> string literal.
      if (!identName.empty() && identName[0] >= 'a' && identName[0] <= 'z') {
        return createTransformedNode<ESTree::StringLiteralNode>(
            name, ident->_name);
      }
      // Uppercase -> identifier reference.
      return makeIdentifierNode(name, ident->_name);
    }

    if (auto *member =
            llvh::dyn_cast<ESTree::JSXMemberExpressionNode>(name)) {
      return buildMemberExpression(member);
    }

    if (auto *nsName = llvh::dyn_cast<ESTree::JSXNamespacedNameNode>(name)) {
      // Namespaced names become strings: "ns:tag"
      auto *ns = llvh::cast<ESTree::JSXIdentifierNode>(nsName->_namespace);
      auto *localName = llvh::cast<ESTree::JSXIdentifierNode>(nsName->_name);
      llvh::SmallString<32> combined;
      combined += ns->_name->str();
      combined += ":";
      combined += localName->_name->str();
      auto *str = context_.getIdentifier(combined).getUnderlyingPointer();
      return createTransformedNode<ESTree::StringLiteralNode>(name, str);
    }

    llvm_unreachable("Unknown JSX element name type");
  }

  /// Build a MemberExpression from JSXMemberExpression.
  ESTree::Node *buildMemberExpression(ESTree::JSXMemberExpressionNode *member) {
    ESTree::Node *object;
    if (auto *objIdent =
            llvh::dyn_cast<ESTree::JSXIdentifierNode>(member->_object)) {
      object = makeIdentifierNode(member->_object, objIdent->_name);
    } else {
      object = buildMemberExpression(
          llvh::cast<ESTree::JSXMemberExpressionNode>(member->_object));
    }

    auto *prop = llvh::cast<ESTree::JSXIdentifierNode>(member->_property);
    auto *property = makeIdentifierNode(member->_property, prop->_name);

    return createTransformedNode<ESTree::MemberExpressionNode>(
        member,
        object,
        property,
        false /* not computed */);
  }

  /// Build the Fragment type expression based on runtime mode.
  ESTree::Node *buildFragmentType(ESTree::Node *srcNode) {
    if (config_.runtime == JSXRuntime::JSX) {
      // JSX.Fragment
      return createTransformedNode<ESTree::MemberExpressionNode>(
          srcNode,
          makeIdentifierNode(srcNode, config_.jsxGlobal),
          makeIdentifierNode(srcNode, "Fragment"),
          false);
    } else {
      // React.Fragment
      return createTransformedNode<ESTree::MemberExpressionNode>(
          srcNode,
          makeIdentifierNode(srcNode, config_.createElementGlobal),
          makeIdentifierNode(srcNode, "Fragment"),
          false);
    }
  }

  /// Collect and transform children from a JSX children list.
  void collectChildren(
      ESTree::NodeList &jsxChildren,
      NodeVector &children,
      bool &hasSpreadChild) {
    for (auto &childNode : jsxChildren) {
      ESTree::Node *child = &childNode;

      if (auto *text = llvh::dyn_cast<ESTree::JSXTextNode>(child)) {
        // Process JSX text with whitespace normalization.
        ESTree::Node *textExpr = processJSXText(text);
        if (textExpr) {
          children.append(textExpr);
        }
        continue;
      }

      if (auto *exprContainer =
              llvh::dyn_cast<ESTree::JSXExpressionContainerNode>(child)) {
        // Unwrap the expression, skip empty expressions.
        if (!llvh::isa<ESTree::JSXEmptyExpressionNode>(
                exprContainer->_expression)) {
          children.append(exprContainer->_expression);
        }
        continue;
      }

      if (auto *spreadChild =
              llvh::dyn_cast<ESTree::JSXSpreadChildNode>(child)) {
        // Spread child in children array.
        hasSpreadChild = true;
        children.append(createTransformedNode<ESTree::SpreadElementNode>(
            spreadChild, spreadChild->_expression));
        continue;
      }

      // JSXElement or JSXFragment - already transformed by visitor.
      children.append(child);
    }
  }

  /// Process JSX text, applying whitespace normalization per the JSX spec.
  /// Returns nullptr if the text is empty after normalization.
  ///
  /// JSX text whitespace normalization rules (matching Babel's implementation):
  /// 1. Lines containing only whitespace are removed entirely
  /// 2. Leading/trailing whitespace on each line is removed
  /// 3. Newlines are converted to spaces
  /// 4. Multiple consecutive whitespace characters collapse to a single space
  /// 5. If the result is empty, the text node is omitted
  ///
  /// Examples:
  ///   "  hello  "        -> "hello"
  ///   "hello\n  world"   -> "hello world"
  ///   "  \n  \n  "       -> (omitted)
  ///   "a   b"            -> "a b"
  ///
  /// Reference: https://facebook.github.io/jsx/ (see "JSXText" section)
  ESTree::Node *processJSXText(ESTree::JSXTextNode *text) {
    llvh::StringRef raw = text->_value->str();

    llvh::SmallString<128> result;
    bool lastWasSpace = true; // Treat beginning as space to trim leading.
    bool hasContent = false;

    for (size_t i = 0; i < raw.size(); ++i) {
      char c = raw[i];
      if (c == '\n' || c == '\r') {
        // Newline becomes space (will be collapsed if adjacent to other space).
        if (!lastWasSpace && hasContent) {
          result += ' ';
          lastWasSpace = true;
        }
      } else if (c == ' ' || c == '\t') {
        if (!lastWasSpace && hasContent) {
          result += ' ';
          lastWasSpace = true;
        }
      } else {
        result += c;
        lastWasSpace = false;
        hasContent = true;
      }
    }

    // Trim trailing space.
    while (!result.empty() && result.back() == ' ') {
      result.pop_back();
    }

    if (result.empty()) {
      return nullptr;
    }

    auto *str = context_.getIdentifier(result).getUnderlyingPointer();
    return createTransformedNode<ESTree::StringLiteralNode>(text, str);
  }

  /// Create a property node for an object literal with a string key.
  ESTree::Node *makePropertyNode(
      ESTree::Node *srcNode,
      llvh::StringRef name,
      ESTree::Node *value) {
    return createTransformedNode<ESTree::PropertyNode>(
        srcNode,
        makeIdentifierNode(srcNode, name),
        value,
        context_.keywords().identInit,
        false /* computed */,
        false /* method */,
        false /* shorthand */);
  }

  /// Create a property node for an object literal with a UniqueString key.
  ESTree::Node *makePropertyNode(
      ESTree::Node *srcNode,
      UniqueString *name,
      ESTree::Node *value) {
    return createTransformedNode<ESTree::PropertyNode>(
        srcNode,
        makeIdentifierNode(srcNode, name),
        value,
        context_.keywords().identInit,
        false /* computed */,
        false /* method */,
        false /* shorthand */);
  }

  /// Build the props object expression from JSX attributes.
  /// Extracts the key attribute for jsx mode.
  /// For createElement dev mode, adds __source and __self props.
  ESTree::Node *buildProps(
      ESTree::Node *srcNode,
      ESTree::NodeList &attributes,
      NodeVector &children,
      bool hasSpreadChild,
      ESTree::Node **keyExpr) {
    *keyExpr = nullptr;
    NodeVector properties;

    for (auto &attrNode : attributes) {
      ESTree::Node *attr = &attrNode;

      if (auto *spreadAttr =
              llvh::dyn_cast<ESTree::JSXSpreadAttributeNode>(attr)) {
        // Spread attribute - add as SpreadElement to props object.
        properties.append(createTransformedNode<ESTree::SpreadElementNode>(
            spreadAttr, spreadAttr->_argument));
        continue;
      }

      auto *jsxAttr = llvh::cast<ESTree::JSXAttributeNode>(attr);
      auto *attrName = getAttributeName(jsxAttr->_name);
      llvh::StringRef attrNameStr = attrName->str();

      // Get attribute value.
      ESTree::Node *value = nullptr;
      if (!jsxAttr->_value) {
        // No value means true: <input disabled />
        value = createTransformedNode<ESTree::BooleanLiteralNode>(jsxAttr, true);
      } else if (auto *strLit = llvh::dyn_cast<ESTree::JSXStringLiteralNode>(
                     jsxAttr->_value)) {
        value =
            createTransformedNode<ESTree::StringLiteralNode>(strLit, strLit->_value);
      } else if (auto *exprContainer =
                     llvh::dyn_cast<ESTree::JSXExpressionContainerNode>(
                         jsxAttr->_value)) {
        value = exprContainer->_expression;
      } else {
        // JSXElement as value (rare but valid).
        value = jsxAttr->_value;
      }

      // In automatic mode, extract key separately.
      if (config_.runtime == JSXRuntime::JSX && attrNameStr == "key") {
        *keyExpr = value;
        continue;
      }

      properties.append(makePropertyNode(jsxAttr->_name, attrName, value));
    }

    // For jsx mode, add children to props.
    if (config_.runtime == JSXRuntime::JSX && children.size() != 0) {
      ESTree::Node *childrenValue;
      if (children.size() == 1 && !hasSpreadChild) {
        childrenValue = *children.begin();
      } else {
        childrenValue = createTransformedNode<ESTree::ArrayExpressionNode>(
            nullptr, children.toNodeList(), false /* trailingComma */);
      }
      properties.append(makePropertyNode(nullptr, "children", childrenValue));
    }

    // For createElement dev mode, add __source and __self props.
    if (config_.runtime == JSXRuntime::CreateElement && config_.development) {
      properties.append(
          makePropertyNode(srcNode, "__source", createSourceObject(srcNode)));
      properties.append(makePropertyNode(
          srcNode,
          "__self",
          createTransformedNode<ESTree::ThisExpressionNode>(srcNode)));
    }

    // If no properties, return null for createElement mode (unless dev mode
    // added props) or empty object for jsx mode.
    if (properties.size() == 0) {
      if (config_.runtime == JSXRuntime::CreateElement) {
        return createTransformedNode<ESTree::NullLiteralNode>(nullptr);
      }
      return createTransformedNode<ESTree::ObjectExpressionNode>(
          nullptr, ESTree::NodeList{});
    }

    return createTransformedNode<ESTree::ObjectExpressionNode>(
        nullptr, properties.toNodeList());
  }

  /// Build props with just children (for fragments).
  ESTree::Node *buildPropsWithChildren(
      ESTree::Node *srcNode,
      NodeVector &children,
      bool hasSpreadChild) {
    if (config_.runtime == JSXRuntime::CreateElement) {
      // Classic mode passes children as arguments, not in props.
      return nullptr;
    }

    if (children.size() == 0) {
      return createTransformedNode<ESTree::ObjectExpressionNode>(
          srcNode, ESTree::NodeList{});
    }

    ESTree::Node *childrenValue;
    if (children.size() == 1 && !hasSpreadChild) {
      childrenValue = *children.begin();
    } else {
      childrenValue = createTransformedNode<ESTree::ArrayExpressionNode>(
          srcNode, children.toNodeList(), false /* trailingComma */);
    }

    NodeVector properties;
    properties.append(makePropertyNode(srcNode, "children", childrenValue));

    return createTransformedNode<ESTree::ObjectExpressionNode>(
        srcNode, properties.toNodeList());
  }

  /// Get the attribute name as a UniqueString.
  UniqueString *getAttributeName(ESTree::Node *name) {
    if (auto *ident = llvh::dyn_cast<ESTree::JSXIdentifierNode>(name)) {
      return ident->_name;
    }
    if (auto *nsName = llvh::dyn_cast<ESTree::JSXNamespacedNameNode>(name)) {
      auto *ns = llvh::cast<ESTree::JSXIdentifierNode>(nsName->_namespace);
      auto *localName = llvh::cast<ESTree::JSXIdentifierNode>(nsName->_name);
      llvh::SmallString<32> combined;
      combined += ns->_name->str();
      combined += ":";
      combined += localName->_name->str();
      return context_.getIdentifier(combined).getUnderlyingPointer();
    }
    llvm_unreachable("Unknown attribute name type");
  }

  /// Create a function call for automatic runtime mode.
  /// JSX.jsx(type, props, key) or JSX.jsxs(type, props, key)
  /// In dev mode: JSX.jsxDEV(type, props, key, isStaticChildren, source, self)
  ESTree::Node *createAutomaticCall(
      ESTree::Node *srcNode,
      ESTree::Node *elementType,
      ESTree::Node *props,
      ESTree::Node *key,
      bool isMultipleChildren) {
    // Build: JSX.jsx or JSX.jsxs or JSX.jsxDEV
    llvh::StringRef funcName;
    if (config_.development) {
      funcName = "jsxDEV";
    } else if (isMultipleChildren) {
      funcName = "jsxs";
    } else {
      funcName = "jsx";
    }

    auto *callee = createTransformedNode<ESTree::MemberExpressionNode>(
        srcNode,
        makeIdentifierNode(srcNode, config_.jsxGlobal),
        makeIdentifierNode(srcNode, funcName),
        false);

    NodeVector args;
    args.append(elementType);
    args.append(props);

    // Key argument (or undefined).
    if (key) {
      args.append(key);
    } else {
      args.append(createTransformedNode<ESTree::UnaryExpressionNode>(
          srcNode,
          context_.getIdentifier("void").getUnderlyingPointer(),
          createTransformedNode<ESTree::NumericLiteralNode>(srcNode, 0.0),
          true));
    }

    // Development mode adds extra arguments.
    if (config_.development) {
      // isStaticChildren boolean
      args.append(createTransformedNode<ESTree::BooleanLiteralNode>(
          srcNode, isMultipleChildren));

      // source object: { fileName, lineNumber, columnNumber }
      args.append(createSourceObject(srcNode));

      // self (this)
      args.append(
          createTransformedNode<ESTree::ThisExpressionNode>(srcNode));
    }

    return createTransformedNode<ESTree::CallExpressionNode>(
        srcNode, callee, nullptr, args.toNodeList());
  }

  /// Create a function call for classic runtime mode.
  /// React.createElement(type, props, ...children)
  ESTree::Node *createClassicCall(
      ESTree::Node *srcNode,
      ESTree::Node *elementType,
      ESTree::Node *props,
      NodeVector &children) {
    // Build: React.createElement
    auto *callee = createTransformedNode<ESTree::MemberExpressionNode>(
        srcNode,
        makeIdentifierNode(srcNode, config_.createElementGlobal),
        makeIdentifierNode(srcNode, "createElement"),
        false);

    NodeVector args;
    args.append(elementType);

    // Props (null if no props).
    if (props) {
      args.append(props);
    } else {
      args.append(createTransformedNode<ESTree::NullLiteralNode>(srcNode));
    }

    // Children as additional arguments.
    for (auto *child : children) {
      args.append(child);
    }

    return createTransformedNode<ESTree::CallExpressionNode>(
        srcNode, callee, nullptr, args.toNodeList());
  }

  /// Create the source object for development mode.
  /// { fileName: "file.js", lineNumber: N, columnNumber: M }
  ESTree::Node *createSourceObject(ESTree::Node *srcNode) {
    NodeVector properties;

    properties.append(makePropertyNode(
        srcNode,
        "fileName",
        createTransformedNode<ESTree::StringLiteralNode>(
            srcNode,
            context_.getIdentifier(sourceFilename_).getUnderlyingPointer())));

    // Get source location if available.
    if (srcNode) {
      SMLoc startLoc = srcNode->getStartLoc();
      if (startLoc.isValid()) {
        SourceErrorManager::SourceCoords coords;
        if (context_.getSourceErrorManager().findBufferLineAndLoc(
                startLoc, coords)) {
          properties.append(makePropertyNode(
              srcNode,
              "lineNumber",
              createTransformedNode<ESTree::NumericLiteralNode>(
                  srcNode, static_cast<double>(coords.line))));

          properties.append(makePropertyNode(
              srcNode,
              "columnNumber",
              createTransformedNode<ESTree::NumericLiteralNode>(
                  srcNode, static_cast<double>(coords.col))));
        }
      }
    }

    return createTransformedNode<ESTree::ObjectExpressionNode>(
        srcNode, properties.toNodeList());
  }
};

} // anonymous namespace

ESTree::Node *transformJSX(
    Context &context,
    ESTree::Node *node,
    const JSXTransformConfig &config,
    llvh::StringRef sourceFilename) {
  JSXTransformer transformer(context, config, sourceFilename);
  visitESTreeNode(transformer, node, nullptr);
  return node;
}

} // namespace hermes
