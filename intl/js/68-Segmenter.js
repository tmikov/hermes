/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Segmenter Objects": the Intl.Segmenter constructor, its
// supportedLocalesOf, the prototype's resolvedOptions, segment and
// @@toStringTag, the Segments objects (CreateSegmentsObject and
// %IntlSegmentsPrototype%), the Segment Iterators (CreateSegmentIterator
// and %IntlSegmentIteratorPrototype%), CreateSegmentDataObject and
// FindBoundary, following the draft https://tc39.es/ecma402/ dated
// October 24, 2026. Each Segmenter has a native ICU4X segmenter of its own,
// attached by segmenterInit. Each Segments object gets the native
// boundaries of its string, computed once by segmentsInit on the first
// containing or next that gets past the draft's early returns, and shared
// by its iterators (Phase 5 plan decisions 9 and 10).

/// %Intl.Segmenter.prototype%; set by installSegmenter.
var SegmenterPrototype;
/// %IntlSegmentsPrototype%; set by installSegmenter.
var SegmentsPrototype;
/// %IntlSegmentIteratorPrototype%; set by installSegmenter.
var SegmentIteratorPrototype;

/// The internal slots of an Intl.Segmenter instance ("Properties of
/// Intl.Segmenter Instances"), as private fields of the object passed to
/// the constructor. [[InitializedSegmenter]] is the presence of #locale.
class SegmenterSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[SegmenterGranularity]]
  #granularity;

  /// Attach the slots to o.
  constructor(o, locale, granularity) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot.
    this.#locale = locale;
    this.#granularity = granularity;
  }

  /// Whether o is an Object with an [[InitializedSegmenter]] internal slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedSegmenter]]). name is the slot's name in lowerCamelCase,
  /// without the "Segmenter" prefix.
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedSegmenter]]).
    requireInitializedSegmenter(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'granularity':
        return o.#granularity;
    }
    throw new _TypeError('Intl internal error: no Segmenter slot ' + name);
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedSegmenter]]).
function requireInitializedSegmenter(o) {
  // 1. If o is not an Object, or has no [[InitializedSegmenter]] internal
  //    slot, throw a TypeError exception.
  if (!SegmenterSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.Segmenter object');
}

/// The internal slots of a Segments instance ("Properties of Segments
/// Instances"), as private fields of the object CreateSegmentsObject
/// makes. #segmented is not a slot of the draft: it records whether
/// segmentsInit has attached the native boundaries of [[SegmentsString]]
/// to the object.
class SegmentsSlots extends ReturnOverride {
  /// [[SegmentsSegmenter]]
  #segmenter;
  /// [[SegmentsString]]
  #string;
  /// Whether the object has its native boundaries.
  #segmented;

  /// Attach the slots to o.
  constructor(o, segmenter, string) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot.
    this.#segmenter = segmenter;
    this.#string = string;
    this.#segmented = false;
  }

  /// Whether o is an Object with a [[SegmentsSegmenter]] internal slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #segmenter in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[SegmentsSegmenter]]). name is the slot's name in lowerCamelCase,
  /// without the "Segments" prefix.
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[SegmentsSegmenter]]).
    requireSegments(o);
    // 2. Return the slot.
    switch (name) {
      case 'segmenter':
        return o.#segmenter;
      case 'string':
        return o.#string;
    }
    throw new _TypeError('Intl internal error: no Segments slot ' + name);
  }

  /// Attach the native boundaries of o's [[SegmentsString]], by
  /// [[SegmentsSegmenter]]'s native segmenter, to the Segments object o,
  /// unless it has them already. FindBoundary and CreateSegmentDataObject
  /// read them.
  static segment(o) {
    // 1. Perform ? RequireInternalSlot(o, [[SegmentsSegmenter]]).
    requireSegments(o);
    // 2. If o has its boundaries, return.
    if (o.#segmented) return;
    // 3. Compute them and attach them to o. #segmenter is an
    //    Intl.Segmenter, whose constructor attached its native segmenter,
    //    and o has no native state yet.
    native.segmentsInit(o, o.#segmenter, o.#string);
    // 4. Record that o has them.
    o.#segmented = true;
  }
}

/// ECMA-262 RequireInternalSlot(o, [[SegmentsSegmenter]]).
function requireSegments(o) {
  // 1. If o is not an Object, or has no [[SegmentsSegmenter]] internal
  //    slot, throw a TypeError exception.
  if (!SegmentsSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.Segments object');
}

/// The internal slots of a Segment Iterator instance (Table "Internal
/// Slots of Segment Iterator Instances"), as private fields of the object
/// CreateSegmentIterator makes. [[IteratingSegmenter]] is not a field: the
/// iterator keeps the Segments object it iterates, whose
/// [[SegmentsSegmenter]] is [[IteratingSegmenter]] and whose native
/// boundaries it reads, which keeps them alive (decision 9). The presence
/// of #segments is the presence of [[IteratingSegmenter]].
class SegmentIteratorSlots extends ReturnOverride {
  /// The Segments object whose [[SegmentsSegmenter]] is
  /// [[IteratingSegmenter]].
  #segments;
  /// [[IteratedString]]
  #string;
  /// [[IteratedStringNextSegmentCodeUnitIndex]]
  #nextIndex;

  /// Attach the slots to o.
  constructor(o, segments, string, nextIndex) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot.
    this.#segments = segments;
    this.#string = string;
    this.#nextIndex = nextIndex;
  }

  /// Whether o is an Object with an [[IteratingSegmenter]] internal slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #segments in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[IteratingSegmenter]]). name is 'segments', 'string' or
  /// 'nextIndex'.
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[IteratingSegmenter]]).
    requireSegmentIterator(o);
    // 2. Return the slot.
    switch (name) {
      case 'segments':
        return o.#segments;
      case 'string':
        return o.#string;
      case 'nextIndex':
        return o.#nextIndex;
    }
    throw new _TypeError(
      'Intl internal error: no Segment Iterator slot ' + name,
    );
  }

  /// Set o.[[IteratedStringNextSegmentCodeUnitIndex]] to nextIndex.
  static setNextIndex(o, nextIndex) {
    // 1. Perform ? RequireInternalSlot(o, [[IteratingSegmenter]]).
    requireSegmentIterator(o);
    // 2. Set the slot.
    o.#nextIndex = nextIndex;
  }
}

/// ECMA-262 RequireInternalSlot(o, [[IteratingSegmenter]]).
function requireSegmentIterator(o) {
  // 1. If o is not an Object, or has no [[IteratingSegmenter]] internal
  //    slot, throw a TypeError exception.
  if (!SegmentIteratorSlots.is(o))
    throw new _TypeError('Receiver is not an Intl Segment Iterator');
}

/// The one record of %Intl.Segmenter%.[[LocaleData]].[[<locale>]] that
/// ResolveLocale sees: there are no relevant extension keys, so it reads no
/// field of it. The segmentation rules are the native segmenter's.
var SEGMENTER_LOCALE_DATA = {__proto__: null};

/// %Intl.Segmenter%.[[LocaleData]].[[<locale>]] for an available locale.
function segmenterLocaleData(locale) {
  // 1. Return the empty record.
  return SEGMENTER_LOCALE_DATA;
}

/// The fields of %Intl.Segmenter% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list,
/// [[RelevantExtensionKeys]] « » and [[ResolutionOptionDescriptors]] « ».
var SEGMENTER_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: [],
  resolutionOptionDescriptors: [],
};

// Intl.Segmenter ( [ locales [ , options ] ] )
function Segmenter(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.Segmenter must be called with new');
  // 2. Let internalSlotsList be « [[InitializedSegmenter]], [[Locale]],
  //    [[SegmenterGranularity]] ».
  // 3. Let segmenter be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.Segmenter.prototype%", internalSlotsList).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"); the slots are attached in step 9, once every value
  //    is known, and nothing between can observe them.
  var segmenter = this;
  // 4. Let optionsResolution be ? ResolveOptions(%Intl.Segmenter%,
  //    %Intl.Segmenter%.[[LocaleData]], locales, options).
  var optionsResolution = ResolveOptions(
    SEGMENTER_SERVICE,
    segmenterLocaleData,
    locales,
    options,
  );
  // 5. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 6. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 7. Set segmenter.[[Locale]] to resolvedLocale.[[Locale]].
  var locale = resolvedLocale.locale;
  // 8. Let granularity be ? GetOption(options, "granularity", string,
  //    « "grapheme", "word", "sentence" », "grapheme").
  var granularity = GetOption(
    options,
    'granularity',
    'string',
    ['grapheme', 'word', 'sentence'],
    'grapheme',
  );
  // 9. Set segmenter.[[SegmenterGranularity]] to granularity.
  new SegmenterSlots(segmenter, locale, granularity);
  //    The native segmenter for granularity, with the language identifier
  //    of [[Locale]] as content locale (decision 9). segmenter is a new
  //    object, so it has no native state yet.
  native.segmenterInit(segmenter, GetLocaleBaseName(locale), granularity);
  // 10. Return segmenter.
  return segmenter;
}

// CreateSegmentsObject ( segmenter, string )
function CreateSegmentsObject(segmenter, string) {
  // 1. Let internalSlotsList be « [[SegmentsSegmenter]],
  //    [[SegmentsString]] ».
  // 2. Let segments be OrdinaryObjectCreate(%IntlSegmentsPrototype%,
  //    internalSlotsList).
  var segments = ObjectCreate(SegmentsPrototype);
  // 3. Set segments.[[SegmentsSegmenter]] to segmenter.
  // 4. Set segments.[[SegmentsString]] to string.
  new SegmentsSlots(segments, segmenter, string);
  // 5. Return segments.
  return segments;
}

// CreateSegmentIterator ( segmenter, string )
// The plugin passes the Segments object segments whose
// [[SegmentsSegmenter]] is segmenter, and keeps it in place of
// [[IteratingSegmenter]] (SegmentIteratorSlots).
function CreateSegmentIterator(segments, string) {
  // 1. Let internalSlotsList be « [[IteratingSegmenter]],
  //    [[IteratedString]], [[IteratedStringNextSegmentCodeUnitIndex]] ».
  // 2. Let iterator be OrdinaryObjectCreate(
  //    %IntlSegmentIteratorPrototype%, internalSlotsList).
  var iterator = ObjectCreate(SegmentIteratorPrototype);
  // 3. Set iterator.[[IteratingSegmenter]] to segmenter.
  // 4. Set iterator.[[IteratedString]] to string.
  // 5. Set iterator.[[IteratedStringNextSegmentCodeUnitIndex]] to 0.
  new SegmentIteratorSlots(iterator, segments, string, 0);
  // 6. Return iterator.
  return iterator;
}

// CreateSegmentDataObject ( segmenter, string, startIndex, endIndex )
// The plugin also passes the Segments object segments whose
// [[SegmentsSegmenter]] is segmenter and whose [[SegmentsString]] is
// string: its native boundaries say whether a word segment is word-like.
function CreateSegmentDataObject(
  segments,
  segmenter,
  string,
  startIndex,
  endIndex,
) {
  // 1. Let length be the length of string.
  // 2. Assert: endIndex ≤ length.
  // 3. Assert: startIndex < endIndex.
  // 4. Let result be OrdinaryObjectCreate(%Object.prototype%).
  // 5. Let segment be the substring of string from startIndex to endIndex.
  var segment = stringSlice(string, startIndex, endIndex);
  // 6. Perform ! CreateDataPropertyOrThrow(result, "segment", segment).
  // 7. Perform ! CreateDataPropertyOrThrow(result, "index",
  //    𝔽(startIndex)).
  // 8. Perform ! CreateDataPropertyOrThrow(result, "input", string).
  // 9. Let granularity be segmenter.[[SegmenterGranularity]].
  var granularity = SegmenterSlots.get(segmenter, 'granularity');
  // 10. If granularity is "word", then
  if (granularity === 'word') {
    // a. Let isWordLike be a Boolean indicating whether the segment in
    //    string is "word-like" according to locale segmenter.[[Locale]].
    //    The native word segmenter of segmenter, which has [[Locale]]'s
    //    language identifier as content locale, said so when it computed
    //    the boundaries of segments; startIndex is one of them, and not
    //    the length.
    var isWordLike = native.segmentsIsWordLike(segments, startIndex);
    // b. Perform ! CreateDataPropertyOrThrow(result, "isWordLike",
    //    isWordLike).
    //    Steps 4 to 8 and 10.b are one object literal, whose properties
    //    are defined in this order without consulting Object.prototype.
    return {
      segment: segment,
      index: startIndex,
      input: string,
      isWordLike: isWordLike,
    };
  }
  // 11. Return result.
  //     Steps 4 to 8 are one object literal, as in step 10.b.
  return {segment: segment, index: startIndex, input: string};
}

// FindBoundary ( segmenter, string, startIndex, direction )
// The plugin passes the Segments object segments whose
// [[SegmentsSegmenter]] is segmenter and whose [[SegmentsString]] is
// string, and direction as 'before' or 'after'. SegmentsSlots.segment has
// attached the native boundaries of segments: every boundary of string, by
// segmenter's [[Locale]] and [[SegmenterGranularity]], ascending, from 0 to
// the length of string.
function FindBoundary(segments, startIndex, direction) {
  // 1. Let length be the length of string.
  // 2. Assert: startIndex < length.
  // 3. Let locale be segmenter.[[Locale]].
  // 4. Let granularity be segmenter.[[SegmenterGranularity]].
  // 5. If direction is before, then
  //    a. Search string for the last segmentation boundary that is
  //       preceded by at most startIndex code units from the beginning,
  //       using locale locale and text element granularity granularity.
  //    b. If a boundary is found, return the count of code units in string
  //       preceding it.
  //    c. Return 0.
  //       The boundaries include 0, so a boundary is always found.
  // 6. Assert: direction is after.
  // 7. Search string for the first segmentation boundary that follows the
  //    code unit at index startIndex, using locale locale and text element
  //    granularity granularity.
  // 8. If a boundary is found, return the count of code units in string
  //    preceding it.
  // 9. Return length.
  //    The boundaries include length, so a boundary is always found.
  //    native.segmentsFindBoundary searches them; 0 ≤ startIndex < length.
  return native.segmentsFindBoundary(
    segments,
    startIndex,
    direction === 'after',
  );
}

// ECMA-262 7.4.17 CreateIteratorResultObject ( value, done )
function CreateIteratorResultObject(value, done) {
  // 1. Let obj be OrdinaryObjectCreate(%Object.prototype%).
  // 2. Perform ! CreateDataPropertyOrThrow(obj, "value", value).
  // 3. Perform ! CreateDataPropertyOrThrow(obj, "done", done).
  // 4. Return obj.
  //    An object literal defines both, in this order, without consulting
  //    Object.prototype.
  return {value: value, done: done};
}

/// The rows of the table "Resolved Options of Segmenter Instances", in
/// table order: [Property, slot name].
var SEGMENTER_RESOLVED_OPTIONS = [
  ['locale', 'locale'],
  ['granularity', 'granularity'],
];

/// Install Intl.Segmenter on intl, and make %IntlSegmentsPrototype% and
/// %IntlSegmentIteratorPrototype%; called by installIntl.
function installSegmenter(intl) {
  // 1. Let proto be %Intl.Segmenter.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.Segmenter Prototype Object").
  var proto = {};
  SegmenterPrototype = proto;
  // 2. "Properties of the Intl.Segmenter Constructor":
  //    Intl.Segmenter.prototype, with its length 0, and
  //    Intl.Segmenter.prototype.constructor.
  defineConstructor(Segmenter, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(Segmenter, {
    // Intl.Segmenter.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be %Intl.Segmenter%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.Segmenter.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let segmenter be the this value.
      var segmenter = this;
      // 2. Perform ? RequireInternalSlot(segmenter,
      //    [[InitializedSegmenter]]).
      requireInitializedSegmenter(segmenter);
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. For each row of Table "Resolved Options of Segmenter Instances",
      //    except the header row, in table order, do
      for (var i = 0; i < SEGMENTER_RESOLVED_OPTIONS.length; i++) {
        var row = SEGMENTER_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        // b. Let value be the value of segmenter's internal slot whose name
        //    is the Internal Slot value of the current row.
        // c. Assert: value is not undefined.
        // d. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
        //    value).
        CreateDataPropertyOrThrow(
          options,
          row[0],
          SegmenterSlots.get(segmenter, row[1]),
        );
      }
      // 5. Return options.
      return options;
    },
    // Intl.Segmenter.prototype.segment ( string )
    segment(string) {
      // 1. Let segmenter be the this value.
      var segmenter = this;
      // 2. Perform ? RequireInternalSlot(segmenter,
      //    [[InitializedSegmenter]]).
      requireInitializedSegmenter(segmenter);
      // 3. Set string to ? ToString(string).
      string = ToString(string);
      // 4. Return CreateSegmentsObject(segmenter, string).
      return CreateSegmentsObject(segmenter, string);
    },
  });
  // 5. Intl.Segmenter.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.Segmenter", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.Segmenter');
  // 6. %IntlSegmentsPrototype%: an ordinary object whose [[Prototype]] is
  //    %Object.prototype%, with containing and %Symbol.iterator%, and no
  //    @@toStringTag ("The %IntlSegmentsPrototype% Object").
  var segmentsProto = {};
  SegmentsPrototype = segmentsProto;
  defineMethods(segmentsProto, {
    // %IntlSegmentsPrototype%.containing ( index )
    containing(index) {
      // 1. Let segments be the this value.
      var segments = this;
      // 2. Perform ? RequireInternalSlot(segments, [[SegmentsSegmenter]]).
      requireSegments(segments);
      // 3. Let segmenter be segments.[[SegmentsSegmenter]].
      var segmenter = SegmentsSlots.get(segments, 'segmenter');
      // 4. Let string be segments.[[SegmentsString]].
      var string = SegmentsSlots.get(segments, 'string');
      // 5. Let length be the length of string.
      var length = string.length;
      // 6. Let n be ? ToIntegerOrInfinity(index).
      var n = ToIntegerOrInfinity(index);
      // 7. If n < 0 or n ≥ length, return undefined.
      if (n < 0 || n >= length) return undefined;
      //    The native boundaries are computed on the first use that gets
      //    here (decision 9).
      SegmentsSlots.segment(segments);
      // 8. Let startIndex be FindBoundary(segmenter, string, n, before).
      var startIndex = FindBoundary(segments, n, 'before');
      // 9. Let endIndex be FindBoundary(segmenter, string, n, after).
      var endIndex = FindBoundary(segments, n, 'after');
      // 10. Return CreateSegmentDataObject(segmenter, string, startIndex,
      //     endIndex).
      return CreateSegmentDataObject(
        segments,
        segmenter,
        string,
        startIndex,
        endIndex,
      );
    },
    // %IntlSegmentsPrototype% [ %Symbol.iterator% ] ( )
    // A concise method with a computed symbol key has the name
    // "[Symbol.iterator]", as the draft requires.
    [SymbolIterator]() {
      // 1. Let segments be the this value.
      var segments = this;
      // 2. Perform ? RequireInternalSlot(segments, [[SegmentsSegmenter]]).
      requireSegments(segments);
      // 3. Let segmenter be segments.[[SegmentsSegmenter]].
      //    The iterator keeps segments, whose slot this is.
      // 4. Let string be segments.[[SegmentsString]].
      var string = SegmentsSlots.get(segments, 'string');
      // 5. Return CreateSegmentIterator(segmenter, string).
      return CreateSegmentIterator(segments, string);
    },
  });
  // 7. %IntlSegmentIteratorPrototype%: an ordinary object whose
  //    [[Prototype]] is %Iterator.prototype% ("The
  //    %IntlSegmentIteratorPrototype% Object").
  var iteratorProto = ObjectCreate(IteratorPrototype);
  SegmentIteratorPrototype = iteratorProto;
  defineMethods(iteratorProto, {
    // %IntlSegmentIteratorPrototype%.next ( )
    next() {
      // 1. Let iterator be the this value.
      var iterator = this;
      // 2. Perform ? RequireInternalSlot(iterator, [[IteratingSegmenter]]).
      requireSegmentIterator(iterator);
      // 3. Let segmenter be iterator.[[IteratingSegmenter]].
      //    It is the [[SegmentsSegmenter]] of the Segments object the
      //    iterator keeps.
      var segments = SegmentIteratorSlots.get(iterator, 'segments');
      var segmenter = SegmentsSlots.get(segments, 'segmenter');
      // 4. Let string be iterator.[[IteratedString]].
      var string = SegmentIteratorSlots.get(iterator, 'string');
      // 5. Let startIndex be
      //    iterator.[[IteratedStringNextSegmentCodeUnitIndex]].
      var startIndex = SegmentIteratorSlots.get(iterator, 'nextIndex');
      // 6. Let length be the length of string.
      var length = string.length;
      // 7. If startIndex ≥ length, then
      //    a. Return CreateIteratorResultObject(undefined, true).
      if (startIndex >= length)
        return CreateIteratorResultObject(undefined, true);
      //    The native boundaries are computed on the first use that gets
      //    here (decision 9).
      SegmentsSlots.segment(segments);
      // 8. Let endIndex be FindBoundary(segmenter, string, startIndex,
      //    after).
      var endIndex = FindBoundary(segments, startIndex, 'after');
      // 9. Set iterator.[[IteratedStringNextSegmentCodeUnitIndex]] to
      //    endIndex.
      SegmentIteratorSlots.setNextIndex(iterator, endIndex);
      // 10. Let segmentData be CreateSegmentDataObject(segmenter, string,
      //     startIndex, endIndex).
      var segmentData = CreateSegmentDataObject(
        segments,
        segmenter,
        string,
        startIndex,
        endIndex,
      );
      // 11. Return CreateIteratorResultObject(segmentData, false).
      return CreateIteratorResultObject(segmentData, false);
    },
  });
  // 8. %IntlSegmentIteratorPrototype% [ %Symbol.toStringTag% ]:
  //    "Segmenter String Iterator", { [[Writable]]: false,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  setToStringTag(iteratorProto, 'Segmenter String Iterator');
  // 9. "The Intl.Segmenter Constructor": the initial value of the
  //    "Segmenter" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'Segmenter', {
    __proto__: null,
    value: Segmenter,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
