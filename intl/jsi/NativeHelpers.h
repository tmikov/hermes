/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

#include <jsi/jsi.h>

#include <cstddef>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace hermes_intl {

/// Conversion tag for NativeHelpers.def: a JS boolean.
struct Bool {};
/// Conversion tag for NativeHelpers.def: a JS string as UTF-8.
struct Utf8 {};
/// Conversion tag for NativeHelpers.def: a JS string or undefined.
struct OptUtf8 {};

/// The C++ type a conversion tag stands for.
template <class Tag>
struct TagType;
/// Bool is a bool.
template <>
struct TagType<Bool> {
  using type = bool;
};
/// Utf8 is a std::string.
template <>
struct TagType<Utf8> {
  using type = std::string;
};
/// OptUtf8 is a std::optional<std::string>; nullopt is undefined.
template <>
struct TagType<OptUtf8> {
  using type = std::optional<std::string>;
};

/// The signature of the C++ function implementing a helper.
template <class Result, class... Args>
using HelperSignature = typename TagType<Result>::type(
    facebook::jsi::Runtime &,
    const typename TagType<Args>::type &...);

namespace detail {

/// Report a plugin bug. Hermes turns a C++ exception from a host function
/// into an Error made from its own intrinsics, which user code cannot
/// intercept.
[[noreturn]] inline void internalError(const std::string &what) {
  throw std::logic_error("Intl internal error: " + what);
}

/// Conversions between JS values and the tags' C++ types.
template <class Tag>
struct Convert;

/// Utf8: a JS string, both ways.
template <>
struct Convert<Utf8> {
  /// The UTF-8 of \p v, which must be a string; \p helper names the caller.
  static std::string fromJS(
      facebook::jsi::Runtime &rt,
      const facebook::jsi::Value &v,
      const char *helper) {
    if (!v.isString())
      internalError(std::string(helper) + ": expected a string");
    return v.getString(rt).utf8(rt);
  }
  /// A JS string from UTF-8.
  static facebook::jsi::Value toJS(
      facebook::jsi::Runtime &rt,
      const std::string &s) {
    return facebook::jsi::String::createFromUtf8(rt, s);
  }
};

/// Bool: a JS boolean (results only).
template <>
struct Convert<Bool> {
  /// The JS boolean \p b.
  static facebook::jsi::Value toJS(facebook::jsi::Runtime &, bool b) {
    return facebook::jsi::Value(b);
  }
};

/// OptUtf8: a JS string or undefined (results only).
template <>
struct Convert<OptUtf8> {
  /// undefined for nullopt, a JS string otherwise.
  static facebook::jsi::Value toJS(
      facebook::jsi::Runtime &rt,
      const std::optional<std::string> &s) {
    if (!s)
      return facebook::jsi::Value::undefined();
    return facebook::jsi::String::createFromUtf8(rt, *s);
  }
};

/// Convert the arguments, call \p fn, convert the result.
template <class Result, class... Args, size_t... I>
facebook::jsi::Value callHelper(
    HelperSignature<Result, Args...> *fn,
    facebook::jsi::Runtime &rt,
    const facebook::jsi::Value *args,
    const char *name,
    std::index_sequence<I...>) {
  return Convert<Result>::toJS(
      rt, fn(rt, Convert<Args>::fromJS(rt, args[I], name)...));
}

} // namespace detail

/// A host function named \p name that calls \p fn. It captures no JS values,
/// so it is safe to destroy on the finalizer thread.
template <class Result, class... Args>
facebook::jsi::Function makeHelper(
    facebook::jsi::Runtime &rt,
    const char *name,
    HelperSignature<Result, Args...> *fn) {
  return facebook::jsi::Function::createFromHostFunction(
      rt,
      facebook::jsi::PropNameID::forAscii(rt, name),
      sizeof...(Args),
      [fn, name](
          facebook::jsi::Runtime &rt,
          const facebook::jsi::Value &,
          const facebook::jsi::Value *args,
          size_t count) -> facebook::jsi::Value {
        if (count != sizeof...(Args))
          detail::internalError(std::string(name) + ": wrong argument count");
        return detail::callHelper<Result, Args...>(
            fn, rt, args, name, std::index_sequence_for<Args...>{});
      });
}

// The two expansions of a NativeHelpers.def line
// HERMES_INTL_HELPER(name, Result, Arg...). They pass Result and the Args on
// as one pack, so a line with no Args expands too.

/// Declare helpers::name, the C++ function implementing the helper.
#define HERMES_INTL_DECLARE_HELPER(name, ...) \
  ::hermes_intl::HelperSignature<__VA_ARGS__> name;

/// Set property \p name of \p object to a host function calling
/// helpers::name, converting its arguments and result.
#define HERMES_INTL_ADD_HELPER(rt, object, name, ...) \
  (object).setProperty(                               \
      rt,                                             \
      #name,                                          \
      ::hermes_intl::makeHelper<__VA_ARGS__>(rt, #name, &helpers::name))

/// One declaration per helper, e.g.
///   std::optional<std::string> canonicalizeLocale(
///       jsi::Runtime &, const std::string &);
namespace helpers {
#define HERMES_INTL_HELPER HERMES_INTL_DECLARE_HELPER
#include "NativeHelpers.def"
#undef HERMES_INTL_HELPER
} // namespace helpers

/// An object with one host function per line of NativeHelpers.def, passed to
/// the JS setup function and never reachable from user code. Its prototype
/// is null, so a name it lacks reads as undefined whatever user code puts on
/// %Object.prototype%.
facebook::jsi::Object makeNativeHelpers(facebook::jsi::Runtime &rt);

/// The defaults object passed to the JS setup function, {locales, timeZone},
/// with a null prototype like makeNativeHelpers's.
facebook::jsi::Object makeSetupDefaults(
    facebook::jsi::Runtime &rt,
    const std::vector<std::string> &locales,
    const std::string &timeZone);

} // namespace hermes_intl
