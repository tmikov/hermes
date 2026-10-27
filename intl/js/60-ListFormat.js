/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "ListFormat Objects": the Intl.ListFormat constructor, its
// supportedLocalesOf, the prototype's resolvedOptions, format,
// formatToParts and @@toStringTag, CreatePartsFromList, FormatList,
// FormatListToParts and StringListFromIterable, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. Each instance has a
// native list formatter of its own, attached by listFormatInit (Phase 5
// plan decisions 2 and 10). CreateListFormat and CreatePartsFromList are
// the internal functions other constructors use (decision 8).

/// %Intl.ListFormat.prototype%; set by installListFormat.
var ListFormatPrototype;

/// The internal slots of an Intl.ListFormat instance ("Properties of
/// Intl.ListFormat Instances"), as private fields of the object passed to
/// the constructor. [[InitializedListFormat]] is the presence of #locale.
/// [[Templates]] is not a field: the native list formatter that
/// listFormatInit attaches holds the templates of [[Locale]], [[Type]] and
/// [[Style]], and chooses the implementation-defined index n of
/// CreatePartsFromList (decision 2).
class ListFormatSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[Type]]
  #type;
  /// [[Style]]
  #style;

  /// Attach the slots to o.
  constructor(o, locale, type, style) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot.
    this.#locale = locale;
    this.#type = type;
    this.#style = style;
  }

  /// Whether o is an Object with an [[InitializedListFormat]] internal
  /// slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedListFormat]]). name is the slot's name in lowerCamelCase.
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedListFormat]]).
    requireInitializedListFormat(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'type':
        return o.#type;
      case 'style':
        return o.#style;
    }
    throw new _TypeError('Intl internal error: no ListFormat slot ' + name);
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedListFormat]]).
function requireInitializedListFormat(o) {
  // 1. If o is not an Object, or has no [[InitializedListFormat]] internal
  //    slot, throw a TypeError exception.
  if (!ListFormatSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.ListFormat object');
}

/// The one record of %Intl.ListFormat%.[[LocaleData]].[[<locale>]] that
/// ResolveLocale sees: there are no relevant extension keys, so it reads no
/// field of it. The templates are the native list formatter's.
var LIST_FORMAT_LOCALE_DATA = {__proto__: null};

/// %Intl.ListFormat%.[[LocaleData]].[[<locale>]] for an available locale.
function listFormatLocaleData(locale) {
  // 1. Return the empty record.
  return LIST_FORMAT_LOCALE_DATA;
}

/// The fields of %Intl.ListFormat% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list,
/// [[RelevantExtensionKeys]] « » and [[ResolutionOptionDescriptors]] « ».
var LIST_FORMAT_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: [],
  resolutionOptionDescriptors: [],
};

/// Steps 3 to 14 of Intl.ListFormat ( [ locales [ , options ] ] ), on the
/// object listFormat that steps 1 and 2 made. The slots are attached at the
/// end, with the native list formatter, once every value is known; nothing
/// between can observe them. Returns listFormat.
function initializeListFormat(listFormat, locales, options) {
  // 3. Let optionsResolution be ? ResolveOptions(%Intl.ListFormat%,
  //    %Intl.ListFormat%.[[LocaleData]], locales, options).
  var optionsResolution = ResolveOptions(
    LIST_FORMAT_SERVICE,
    listFormatLocaleData,
    locales,
    options,
  );
  // 4. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 5. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 6. Set listFormat.[[Locale]] to resolvedLocale.[[Locale]].
  var locale = resolvedLocale.locale;
  // 7. Let type be ? GetOption(options, "type", string, « "conjunction",
  //    "disjunction", "unit" », "conjunction").
  var type = GetOption(
    options,
    'type',
    'string',
    ['conjunction', 'disjunction', 'unit'],
    'conjunction',
  );
  // 8. Set listFormat.[[Type]] to type.
  // 9. Let style be ? GetOption(options, "style", string, « "long",
  //    "short", "narrow" », "long").
  var style = GetOption(
    options,
    'style',
    'string',
    ['long', 'short', 'narrow'],
    'long',
  );
  // 10. Set listFormat.[[Style]] to style.
  new ListFormatSlots(listFormat, locale, type, style);
  // 11. Let resolvedLocaleData be resolvedLocale.[[LocaleData]].
  // 12. Let dataLocaleTypes be resolvedLocaleData.[[<type>]].
  // 13. Set listFormat.[[Templates]] to dataLocaleTypes.[[<style>]].
  //     The templates are ICU4X's for the language identifier of [[Locale]],
  //     type and style, which listFormatInit attaches. listFormat is a new
  //     object, so it has no native state yet.
  native.listFormatInit(listFormat, GetLocaleBaseName(locale), type, style);
  // 14. Return listFormat.
  return listFormat;
}

// Intl.ListFormat ( [ locales [ , options ] ] )
function ListFormat(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.ListFormat must be called with new');
  // 2. Let listFormat be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.ListFormat.prototype%", « [[InitializedListFormat]],
  //    [[Locale]], [[Type]], [[Style]], [[Templates]] »).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"); initializeListFormat attaches the slots.
  // 3-14. The steps that resolve the slots, numbered there.
  return initializeListFormat(this, locales, options);
}

/// The result of Construct(%Intl.ListFormat%, « locales, options ») for the
/// plugin's own callers, such as DurationFormat (decision 8): every step of
/// the constructor, with NewTarget %Intl.ListFormat%, without going
/// through any public property.
function CreateListFormat(locales, options) {
  // 1-2. Let listFormat be OrdinaryObjectCreate(
  //      %Intl.ListFormat.prototype%).
  // 3-14. Initialize it and return it.
  return initializeListFormat(
    ObjectCreate(ListFormatPrototype),
    locales,
    options,
  );
}

/// Append the part { [[Type]]: "literal", [[Value]]: literal } to parts,
/// unless literal is empty: PartitionPattern, which DeconstructPattern runs
/// on a template, makes no part for empty literal text.
function pushListLiteral(parts, literal) {
  // 1. If literal is not the empty String, append the Record.
  if (literal !== '')
    listPush(parts, {__proto__: null, type: 'literal', value: literal});
}

// CreatePartsFromList ( listFormat, list )
// Returns a List of null-prototype records { type, value }. listFormat's
// native list formatter chooses each template, its index n, and the
// literal text of DeconstructPattern; it sees only the last element of
// list, the only one ICU4X's conditional templates test (decision 2).
// ICU4X's templates have the elements in order, nothing before {0} in
// [[End]] and nothing after {1} in [[Start]], and [[Middle]] is
// "{0}<text>{1}", so the nested result of the draft's steps 4 to 7 is the
// elements in order, with the five pieces of listFormatLiterals around
// them.
function CreatePartsFromList(listFormat, list) {
  // 1. Let size be the number of elements of list.
  var size = list.length;
  var parts = [];
  // 2. If size is 0, then
  //    a. Return a new empty List.
  if (size === 0) return parts;
  // 3. If size is 2, then
  //    a. Let n be an index into listFormat.[[Templates]] based on
  //       listFormat.[[Locale]], list[0], and list[1].
  //    b. Let pattern be listFormat.[[Templates]][n].[[Pair]].
  //    c-e. Let placeables be the Record { [[0]]: first, [[1]]: second },
  //         first and second element Records of list[0] and list[1].
  //    f. Return DeconstructPattern(pattern, placeables).
  //    native.listFormatLiterals gives the literal text before, between
  //    and after the two elements.
  if (size === 2) {
    var pair = native.listFormatLiterals(listFormat, true, list[1]);
    pushListLiteral(parts, pair[0]);
    listPush(parts, {__proto__: null, type: 'element', value: list[0]});
    pushListLiteral(parts, pair[1]);
    listPush(parts, {__proto__: null, type: 'element', value: list[1]});
    pushListLiteral(parts, pair[2]);
    return parts;
  }
  // 4. Let last be the Record { [[Type]]: "element", [[Value]]:
  //    list[size - 1] }.
  // 5. Let parts be « last ».
  //    For size 1, the loop of step 7 does not run, so the result is
  //    « last ».
  if (size === 1) {
    listPush(parts, {__proto__: null, type: 'element', value: list[0]});
    return parts;
  }
  // 6. Let i be size - 2.
  // 7. Repeat, while i ≥ 0,
  //    a. Let head be the Record { [[Type]]: "element", [[Value]]: list[i] }.
  //    b. Let n be an implementation-defined index into
  //       listFormat.[[Templates]] based on listFormat.[[Locale]], head, and
  //       parts.
  //    c. If i is 0, let pattern be listFormat.[[Templates]][n].[[Start]].
  //    d. Else if i is less than size - 2, let pattern be
  //       listFormat.[[Templates]][n].[[Middle]].
  //    e. Else, let pattern be listFormat.[[Templates]][n].[[End]].
  //    f. Let placeables be the Record { [[0]]: head, [[1]]: parts }.
  //    g. Set parts to DeconstructPattern(pattern, placeables).
  //    h. Set i to i - 1.
  //    native.listFormatLiterals gives the literal text of the flattened
  //    result: 0 before list[0] and 1 after it ([[Start]]), 2 between two
  //    elements in the middle ([[Middle]]), 3 before list[size - 1] and 4
  //    after it ([[End]]).
  var literals = native.listFormatLiterals(listFormat, false, list[size - 1]);
  pushListLiteral(parts, literals[0]);
  listPush(parts, {__proto__: null, type: 'element', value: list[0]});
  pushListLiteral(parts, literals[1]);
  listPush(parts, {__proto__: null, type: 'element', value: list[1]});
  for (var i = 2; i < size - 1; i++) {
    pushListLiteral(parts, literals[2]);
    listPush(parts, {__proto__: null, type: 'element', value: list[i]});
  }
  pushListLiteral(parts, literals[3]);
  listPush(parts, {__proto__: null, type: 'element', value: list[size - 1]});
  pushListLiteral(parts, literals[4]);
  // 8. Return parts.
  return parts;
}

// FormatList ( listFormat, list )
function FormatList(listFormat, list) {
  // 1. Let parts be CreatePartsFromList(listFormat, list).
  var parts = CreatePartsFromList(listFormat, list);
  // 2. Let result be the empty String.
  var result = '';
  // 3. For each Record { [[Type]], [[Value]] } part of parts, do
  //    a. Set result to the string-concatenation of result and
  //       part.[[Value]].
  for (var i = 0; i < parts.length; i++) result += parts[i].value;
  // 4. Return result.
  return result;
}

// FormatListToParts ( listFormat, list )
function FormatListToParts(listFormat, list) {
  // 1. Let parts be CreatePartsFromList(listFormat, list).
  var parts = CreatePartsFromList(listFormat, list);
  // 2. Let result be ! ArrayCreate(0).
  var result = [];
  // 3. Let n be 0.
  // 4. For each Record { [[Type]], [[Value]] } part of parts, do
  for (var n = 0; n < parts.length; n++) {
    // a. Let obj be OrdinaryObjectCreate(%Object.prototype%).
    // b. Perform ! CreateDataPropertyOrThrow(obj, "type", part.[[Type]]).
    // c. Perform ! CreateDataPropertyOrThrow(obj, "value", part.[[Value]]).
    //    An object literal defines both, in this order.
    // d. Perform ! CreateDataPropertyOrThrow(result, ! ToString(𝔽(n)),
    //    obj).
    // e. Set n to n + 1.
    listPush(result, {type: parts[n].type, value: parts[n].value});
  }
  // 5. Return result.
  return result;
}

// StringListFromIterable ( iterable )
function StringListFromIterable(iterable) {
  // 1. If iterable is undefined, then
  //    a. Return a new empty List.
  if (iterable === undefined) return [];
  // 2. Let iteratorRecord be ? GetIterator(iterable, sync).
  var iteratorRecord = GetIterator(iterable);
  // 3. Let list be a new empty List.
  var list = [];
  // 4. Repeat,
  for (;;) {
    // a. Let next be ? IteratorStepValue(iteratorRecord).
    var next = IteratorStepValue(iteratorRecord);
    // b. If next is done, then
    //    i. Return list.
    if (next === ITERATOR_DONE) return list;
    // c. If next is not a String, then
    //    i. Let error be ThrowCompletion(a newly created TypeError object).
    //    ii. Return ? IteratorClose(iteratorRecord, error).
    if (typeof next !== 'string')
      return IteratorCloseOnThrow(
        iteratorRecord,
        new _TypeError('Intl.ListFormat: the iterable yielded a non-string'),
      );
    // d. Append next to list.
    listPush(list, next);
  }
}

/// The rows of the table "Resolved Options of ListFormat Instances", in
/// table order: [Property, slot name].
var LIST_FORMAT_RESOLVED_OPTIONS = [
  ['locale', 'locale'],
  ['type', 'type'],
  ['style', 'style'],
];

/// Install Intl.ListFormat on intl; called by installIntl.
function installListFormat(intl) {
  // 1. Let proto be %Intl.ListFormat.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.ListFormat Prototype Object").
  var proto = {};
  ListFormatPrototype = proto;
  // 2. "Properties of the Intl.ListFormat Constructor":
  //    Intl.ListFormat.prototype, with its length 0, and
  //    Intl.ListFormat.prototype.constructor.
  defineConstructor(ListFormat, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(ListFormat, {
    // Intl.ListFormat.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be %Intl.ListFormat%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.ListFormat.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let lf be the this value.
      var lf = this;
      // 2. Perform ? RequireInternalSlot(lf, [[InitializedListFormat]]).
      requireInitializedListFormat(lf);
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. For each row of Table "Resolved Options of ListFormat
      //    Instances", except the header row, in table order, do
      for (var i = 0; i < LIST_FORMAT_RESOLVED_OPTIONS.length; i++) {
        var row = LIST_FORMAT_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        // b. Let value be the value of lf's internal slot whose name is the
        //    Internal Slot value of the current row.
        // c. Assert: value is not undefined.
        // d. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
        //    value).
        CreateDataPropertyOrThrow(
          options,
          row[0],
          ListFormatSlots.get(lf, row[1]),
        );
      }
      // 5. Return options.
      return options;
    },
    // Intl.ListFormat.prototype.format ( list )
    format(list) {
      // 1. Let lf be the this value.
      var lf = this;
      // 2. Perform ? RequireInternalSlot(lf, [[InitializedListFormat]]).
      requireInitializedListFormat(lf);
      // 3. Let stringList be ? StringListFromIterable(list).
      var stringList = StringListFromIterable(list);
      // 4. Return FormatList(lf, stringList).
      return FormatList(lf, stringList);
    },
    // Intl.ListFormat.prototype.formatToParts ( list )
    formatToParts(list) {
      // 1. Let lf be the this value.
      var lf = this;
      // 2. Perform ? RequireInternalSlot(lf, [[InitializedListFormat]]).
      requireInitializedListFormat(lf);
      // 3. Let stringList be ? StringListFromIterable(list).
      var stringList = StringListFromIterable(list);
      // 4. Return FormatListToParts(lf, stringList).
      return FormatListToParts(lf, stringList);
    },
  });
  // 5. Intl.ListFormat.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.ListFormat", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.ListFormat');
  // 6. "The Intl.ListFormat Constructor": the initial value of the
  //    "ListFormat" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'ListFormat', {
    __proto__: null,
    value: ListFormat,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
