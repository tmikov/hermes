/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Collator Objects": the Intl.Collator constructor, its
// supportedLocalesOf, the prototype's resolvedOptions, compare and
// @@toStringTag, and CompareStrings, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026.

/// %Intl.Collator.prototype%; set by installCollator.
var CollatorPrototype;

/// The internal slots of an Intl.Collator instance ("Properties of
/// Intl.Collator Instances"), as private fields of the object passed to the
/// constructor. [[InitializedCollator]] is the presence of #locale.
class CollatorSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[Usage]]
  #usage;
  /// [[Collation]]
  #collation;
  /// [[Numeric]]
  #numeric;
  /// [[CaseFirst]]
  #caseFirst;
  /// [[Sensitivity]]
  #sensitivity;
  /// [[IgnorePunctuation]]
  #ignorePunctuation;
  /// [[BoundCompare]]
  #boundCompare;

  /// Attach the slots to o, from a null-prototype record whose fields are
  /// named after the slots. [[BoundCompare]] starts undefined.
  constructor(o, record) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = record.locale;
    this.#usage = record.usage;
    this.#collation = record.collation;
    this.#numeric = record.numeric;
    this.#caseFirst = record.caseFirst;
    this.#sensitivity = record.sensitivity;
    this.#ignorePunctuation = record.ignorePunctuation;
    this.#boundCompare = undefined;
  }

  /// Whether o is an Object with an [[InitializedCollator]] internal slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if ((typeof o !== 'object' || o === null) && typeof o !== 'function')
      return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedCollator]]).
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedCollator]]).
    requireInitializedCollator(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'usage':
        return o.#usage;
      case 'collation':
        return o.#collation;
      case 'numeric':
        return o.#numeric;
      case 'caseFirst':
        return o.#caseFirst;
      case 'sensitivity':
        return o.#sensitivity;
      case 'ignorePunctuation':
        return o.#ignorePunctuation;
      case 'boundCompare':
        return o.#boundCompare;
    }
    throw new _TypeError('Intl internal error: no Collator slot ' + name);
  }

  /// Set o.[[BoundCompare]] to f, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedCollator]]).
  static setBoundCompare(o, f) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedCollator]]).
    requireInitializedCollator(o);
    // 2. Set o.[[BoundCompare]] to f.
    o.#boundCompare = f;
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedCollator]]).
function requireInitializedCollator(o) {
  // 1. If o is not an Object, or has no [[InitializedCollator]] internal
  //    slot, throw a TypeError exception.
  if (!CollatorSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.Collator object');
}

/// The values of a [[kf]] list, in the order the other two follow the
/// locale's default.
var CASE_FIRST_VALUES = ['upper', 'lower', 'false'];

/// The fields of %Intl.Collator%.[[SortLocaleData]].[[<locale>]] for an
/// available locale (design spec, "Data", Collator): [[co]], [[kn]], [[kf]]
/// and [[ignorePunctuation]].
function collatorLocaleData(locale) {
  // 1. Let co be « null » followed by the collations of locale, the list
  //    Intl.Locale.prototype.getCollations uses.
  var co = [null];
  var collations = splitSpaces(native.collationsOfLocale(locale));
  for (var i = 0; i < collations.length; i++) listPush(co, collations[i]);
  // 2. Let defaults be ICU4X's « caseFirst, ignorePunctuation » for locale.
  //    This relies on the contract of collatorLocaleDefaults in
  //    jsi/NativeHelpers.def: exactly two space-separated tokens, the first
  //    one of "upper", "lower" or "false", the second "true" or "false".
  var defaults = splitSpaces(native.collatorLocaleDefaults(locale));
  // 3. Let kf be « the default caseFirst », followed by the other values of
  //    « "upper", "lower", "false" » in that order.
  var kf = [defaults[0]];
  for (var j = 0; j < CASE_FIRST_VALUES.length; j++) {
    if (CASE_FIRST_VALUES[j] !== defaults[0])
      listPush(kf, CASE_FIRST_VALUES[j]);
  }
  // 4. Return the Record { [[co]]: co, [[kn]]: « "false", "true" »,
  //    [[kf]]: kf, [[ignorePunctuation]]: whether the default alternate
  //    handling is shifted }.
  return {
    __proto__: null,
    co: co,
    kn: ['false', 'true'],
    kf: kf,
    ignorePunctuation: defaults[1] === 'true',
  };
}

/// The records collatorSortLocaleData has returned, keyed by locale. A
/// locale's data cannot change while the runtime lives, and computing it
/// builds an ICU4X collator for each tailoring the locale might have, so
/// each record is computed once and shared. Nothing modifies a record or
/// its lists. There is at most one entry per available locale.
var collatorLocaleDataCache = {__proto__: null};

/// %Intl.Collator%.[[SortLocaleData]].[[<locale>]] ("Internal slots").
function collatorSortLocaleData(locale) {
  // 1. Let data be the cached locale data of locale.
  var data = collatorLocaleDataCache[locale];
  // 2. If there is none, compute it and cache it.
  if (data === undefined) {
    data = collatorLocaleData(locale);
    collatorLocaleDataCache[locale] = data;
  }
  // 3. Return data.
  return data;
}

/// %Intl.Collator%.[[SearchLocaleData]].[[<locale>]] ("Internal slots"):
/// the sort data, since ICU4X's compiled data has no search collations
/// (design spec, "Data", Collator), with [[sensitivity]] "variant".
function collatorSearchLocaleData(locale) {
  // 1. Let data be the sort data of locale.
  var data = collatorSortLocaleData(locale);
  // 2. Return a copy of data with [[sensitivity]] "variant". The shared
  //    sort record itself is left unchanged.
  return {
    __proto__: null,
    co: data.co,
    kn: data.kn,
    kf: data.kf,
    ignorePunctuation: data.ignorePunctuation,
    sensitivity: 'variant',
  };
}

/// The fields of %Intl.Collator% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list, and
/// [[RelevantExtensionKeys]] « "co", "kf", "kn" » (both in design spec,
/// "Data", Collator), and the draft's [[ResolutionOptionDescriptors]].
var COLLATOR_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: ['co', 'kf', 'kn'],
  resolutionOptionDescriptors: [
    {__proto__: null, key: 'co', property: 'collation'},
    {__proto__: null, key: 'kn', property: 'numeric', type: 'boolean'},
    {
      __proto__: null,
      key: 'kf',
      property: 'caseFirst',
      values: ['upper', 'lower', 'false'],
    },
  ],
};

/// The steps of Intl.Collator ( [ locales [ , options ] ] ) that user code
/// can observe: steps 5 to 7, the reads of step 11's ResolveOptions, and
/// the reads of steps 20 and 22. Returns the values they read, as the
/// null-prototype record { usage, resolveInputs (what readResolveOptions
/// returned), sensitivity, ignorePunctuation }, the last two undefined
/// when the option is absent. resolveCollatorSlots finishes the steps from
/// the record alone, so readCollatorOptions followed by
/// resolveCollatorSlots performs steps 4 to 22 in order.
function readCollatorOptions(locales, options) {
  // 4. NOTE: The source of locale data for ResolveOptions depends upon the
  //    "usage" property of options, but the following two steps must
  //    observably precede that lookup (and must not observably repeat
  //    inside ResolveOptions).
  // 5. Let requestedLocales be ? CanonicalizeLocaleList(locales).
  var requestedLocales = CanonicalizeLocaleList(locales);
  // 6. Set options to ? CoerceOptionsToObject(options).
  options = CoerceOptionsToObject(options);
  // 7. Let usage be ? GetOption(options, "usage", string, « "sort",
  //    "search" », "sort").
  var usage = GetOption(options, 'usage', 'string', ['sort', 'search'], 'sort');
  // 11. Let optionsResolution be ? ResolveOptions(%Intl.Collator%,
  //     localeData, CreateArrayFromList(requestedLocales), options).
  //     Steps 1 to 7 of ResolveOptions, which do not use localeData, are
  //     the observable ones; resolveCollatorSlots does the rest. options is
  //     an Object now, so ResolveOptions's GetOptionsObject returns it
  //     unchanged, and canonicalizing the canonical list again reads only
  //     its own elements.
  var resolveInputs = readResolveOptions(
    COLLATOR_SERVICE,
    CreateArrayFromList(requestedLocales),
    options,
  );
  // 20. Set collator.[[Sensitivity]] to ? GetOption(options, "sensitivity",
  //     string, « "base", "accent", "case", "variant" », defaultSensitivity).
  //     GetOption returns its default exactly when the property is
  //     undefined, so the option is read here with the default undefined,
  //     and resolveCollatorSlots puts in defaultSensitivity.
  var sensitivity = GetOption(
    options,
    'sensitivity',
    'string',
    ['base', 'accent', 'case', 'variant'],
    undefined,
  );
  // 22. Set collator.[[IgnorePunctuation]] to ? GetOption(options,
  //     "ignorePunctuation", boolean, empty, defaultIgnorePunctuation).
  //     Read the same way as sensitivity.
  var ignorePunctuation = GetOption(
    options,
    'ignorePunctuation',
    'boolean',
    undefined,
    undefined,
  );
  //     Return what steps 5 to 22 read.
  return {
    __proto__: null,
    usage: usage,
    resolveInputs: resolveInputs,
    sensitivity: sensitivity,
    ignorePunctuation: ignorePunctuation,
  };
}

/// The steps of Intl.Collator ( [ locales [ , options ] ] ) from 8 to 22
/// that depend only on what readCollatorOptions read (the record inputs)
/// and on DefaultLocale(). None of them is observable. Returns the values
/// of the slots, as a null-prototype record whose fields are named after
/// the slots (as CollatorSlots takes it).
function resolveCollatorSlots(inputs) {
  var usage = inputs.usage;
  // 8. Set collator.[[Usage]] to usage.
  var slots = {__proto__: null, usage: usage};
  var localeData;
  if (usage === 'sort') {
    // 9. If usage is "sort", then
    //    a. Let localeData be %Intl.Collator%.[[SortLocaleData]].
    localeData = collatorSortLocaleData;
  } else {
    // 10. Else,
    //     a. Let localeData be %Intl.Collator%.[[SearchLocaleData]].
    localeData = collatorSearchLocaleData;
  }
  // 11. Let optionsResolution be ? ResolveOptions(%Intl.Collator%,
  //     localeData, CreateArrayFromList(requestedLocales), options): steps
  //     8 to 10 of ResolveOptions, on what readCollatorOptions read.
  var optionsResolution = resolveFromOptions(
    COLLATOR_SERVICE,
    localeData,
    inputs.resolveInputs,
  );
  // 12. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 13. Set collator.[[Locale]] to resolvedLocale.[[Locale]].
  slots.locale = resolvedLocale.locale;
  // 14. If resolvedLocale.[[co]] is null, let collation be "default".
  //     Otherwise, let collation be resolvedLocale.[[co]].
  var collation = resolvedLocale.co === null ? 'default' : resolvedLocale.co;
  // 15. Set collator.[[Collation]] to collation.
  slots.collation = collation;
  // 16. Set collator.[[Numeric]] to SameValue(resolvedLocale.[[kn]],
  //     "true").
  slots.numeric = resolvedLocale.kn === 'true';
  // 17. Set collator.[[CaseFirst]] to resolvedLocale.[[kf]].
  slots.caseFirst = resolvedLocale.kf;
  // 18. Let resolvedLocaleData be resolvedLocale.[[LocaleData]].
  var resolvedLocaleData = resolvedLocale.localeData;
  // 19. If usage is "sort", let defaultSensitivity be "variant". Otherwise,
  //     let defaultSensitivity be resolvedLocaleData.[[sensitivity]].
  var defaultSensitivity =
    usage === 'sort' ? 'variant' : resolvedLocaleData.sensitivity;
  // 20. Set collator.[[Sensitivity]] to ? GetOption(options, "sensitivity",
  //     string, « "base", "accent", "case", "variant" », defaultSensitivity):
  //     the value read, or defaultSensitivity if there was none.
  slots.sensitivity =
    inputs.sensitivity === undefined ? defaultSensitivity : inputs.sensitivity;
  // 21. Let defaultIgnorePunctuation be
  //     resolvedLocaleData.[[ignorePunctuation]].
  var defaultIgnorePunctuation = resolvedLocaleData.ignorePunctuation;
  // 22. Set collator.[[IgnorePunctuation]] to ? GetOption(options,
  //     "ignorePunctuation", boolean, empty, defaultIgnorePunctuation): the
  //     value read, or defaultIgnorePunctuation if there was none.
  slots.ignorePunctuation =
    inputs.ignorePunctuation === undefined
      ? defaultIgnorePunctuation
      : inputs.ignorePunctuation;
  //     Return the slots of steps 8 to 22.
  return slots;
}

/// Attach to collator the slots of the record slots that
/// resolveCollatorSlots returned, then the native collator for them
/// (design spec "Native state"). Returns collator.
function attachCollatorSlots(collator, slots) {
  // 1. Attach the slots.
  new CollatorSlots(collator, slots);
  // 2. Attach the native collator for them.
  native.collatorInit(
    collator,
    slots.locale,
    slots.collation,
    slots.sensitivity,
    slots.ignorePunctuation,
    slots.numeric,
    slots.caseFirst,
  );
  // 3. Return collator.
  return collator;
}

/// Steps 4 to 23 of Intl.Collator ( [ locales [ , options ] ] ), on the
/// object collator that steps 1 to 3 made. The slots and the native
/// collator are attached at the end, once every value is known; nothing
/// between can observe them.
function initializeCollator(collator, locales, options) {
  // 4-22. Read the options, then resolve the slots from them; the steps
  //       are numbered there.
  var slots = resolveCollatorSlots(readCollatorOptions(locales, options));
  //     Attach the slots of steps 8 to 22 to collator, and the native
  //     collator.
  attachCollatorSlots(collator, slots);
  // 23. Return collator.
  return collator;
}

// Intl.Collator ( [ locales [ , options ] ] )
function Collator(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, let newTarget be the active function
  //    object, else let newTarget be NewTarget.
  // 2. Let internalSlotsList be « [[InitializedCollator]], [[Locale]],
  //    [[Usage]], [[Collation]], [[Numeric]], [[CaseFirst]],
  //    [[Sensitivity]], [[IgnorePunctuation]], [[BoundCompare]] ».
  //    CollatorSlots has all of them.
  // 3. Let collator be ? OrdinaryCreateFromConstructor(newTarget,
  //    "%Intl.Collator.prototype%", internalSlotsList).
  //    With new, Hermes created this from NewTarget before the call (design
  //    spec "Constructors"). Without new, newTarget is %Intl.Collator%,
  //    whose "prototype" is the non-writable, non-configurable
  //    %Intl.Collator.prototype%, so the object is made from that.
  //    initializeCollator attaches the slots.
  var collator =
    new.target === undefined ? ObjectCreate(CollatorPrototype) : this;
  // 4-23. The remaining steps, numbered there.
  return initializeCollator(collator, locales, options);
}

/// How many collators internalCollator keeps. When the cache holds this
/// many and another is needed, the cache is emptied first, so a program
/// that compares with ever new options keeps at most this many native
/// collators alive. 32 covers the handful of option sets a program
/// typically uses.
var INTERNAL_COLLATOR_CACHE_SIZE = 32;

/// The collators internalCollator has made, keyed by what was read to make
/// them (see internalCollatorKey), and how many there are (design spec
/// "Resistance to tampering").
var internalCollators = {__proto__: null};
var internalCollatorCount = 0;

/// The cache key of the record inputs that readCollatorOptions returned:
/// the requested locales separated by ",", then ";" and each value read,
/// in the order the draft reads them, separated by ";": usage,
/// localeMatcher, the resolution options (collation, numeric, caseFirst),
/// sensitivity and ignorePunctuation. An absent option is the empty
/// String. No requested locale or value read contains "," or ";" or is
/// empty: the locales are canonical language tags, collation is a Unicode
/// type, numeric and ignorePunctuation are booleans (numeric as a String),
/// and the rest are from fixed lists. So two records have the same key
/// only if they hold the same values.
function internalCollatorKey(inputs) {
  var resolveInputs = inputs.resolveInputs;
  // 1. Let key be the requested locales, separated by ",".
  var requestedLocales = resolveInputs.requestedLocales;
  var key = '';
  for (var i = 0; i < requestedLocales.length; i++)
    key += (i === 0 ? '' : ',') + requestedLocales[i];
  // 2. Append usage and localeMatcher.
  var opt = resolveInputs.opt;
  key += ';' + inputs.usage + ';' + opt.localeMatcher;
  // 3. Append the resolution options, in the order of the descriptors.
  var descriptors = COLLATOR_SERVICE.resolutionOptionDescriptors;
  for (var j = 0; j < descriptors.length; j++) {
    var value = opt[descriptors[j].key];
    key += ';' + (value === undefined ? '' : value);
  }
  // 4. Append sensitivity and ignorePunctuation, and return key.
  var sensitivity = inputs.sensitivity;
  var ignorePunctuation = inputs.ignorePunctuation;
  key += ';' + (sensitivity === undefined ? '' : sensitivity);
  key += ';' + (ignorePunctuation === undefined ? '' : ignorePunctuation);
  return key;
}

/// The result of Construct(%Intl.Collator%, « locales, options ») for the
/// built-ins that use a collator internally and never expose it to user
/// code (design spec "Resistance to tampering"). It performs every
/// observable step of the constructor, without going through
/// %Intl.Collator% or any public property, then returns a collator made
/// from %Intl.Collator.prototype% with the slots those steps give. The
/// rest of the constructor depends only on what was read and on
/// DefaultLocale(), and ECMA-402 notes that such a collator "need not
/// actually exist" ("Locale Sensitive Functions of the ECMAScript Language
/// Specification"), so one collator, with its resolution, is shared among
/// all the calls that read the same values.
function internalCollator(locales, options) {
  // 1. Let inputs be what the observable steps of Intl.Collator read for
  //    locales and options; every observable step happens here.
  var inputs = readCollatorOptions(locales, options);
  // 2. Let key be the cache key of inputs.
  var key = internalCollatorKey(inputs);
  // 3. If the cache has a collator for key, return it.
  var collator = internalCollators[key];
  if (collator !== undefined) return collator;
  // 4. If the cache is full, empty it.
  if (internalCollatorCount >= INTERNAL_COLLATOR_CACHE_SIZE) {
    internalCollators = {__proto__: null};
    internalCollatorCount = 0;
  }
  // 5. Let collator be OrdinaryObjectCreate(%Intl.Collator.prototype%),
  //    with the slots resolved from inputs and a native collator attached.
  collator = attachCollatorSlots(
    ObjectCreate(CollatorPrototype),
    resolveCollatorSlots(inputs),
  );
  // 6. Add collator to the cache under key, and return it.
  internalCollators[key] = collator;
  internalCollatorCount++;
  return collator;
}

// CompareStrings ( collator, x, y )
function CompareStrings(collator, x, y) {
  // 1. Return the implementation-defined locale-sensitive comparison of x
  //    with y for collator's resolved options: -1, 0 or 1 from the native
  //    collator, which reads unpaired surrogates as U+FFFD.
  return native.collatorCompare(collator, x, y);
}

/// The bound compare function of collator: step 3 of get
/// Intl.Collator.prototype.compare. An arrow function returned from here
/// has the name "" and length 2, and is not a constructor.
function makeBoundCompare(collator) {
  // a. Let closure be a new Abstract Closure with parameters (x, y) that
  //    captures collator and performs the following steps when called:
  // b. Set collator.[[BoundCompare]] to CreateBuiltinFunction(closure, 2,
  //    "", « »). (The caller does this.)
  return (x, y) => {
    // i. Let xString be ? ToString(x).
    var xString = ToString(x);
    // ii. Let yString be ? ToString(y).
    var yString = ToString(y);
    // iii. Return CompareStrings(collator, xString, yString).
    return CompareStrings(collator, xString, yString);
  };
}

/// Install Intl.Collator on intl; called by installIntl.
function installCollator(intl) {
  // 1. Let proto be %Intl.Collator.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the Intl.Collator
  //    Prototype Object").
  var proto = {};
  CollatorPrototype = proto;
  // 2. "Properties of the Intl.Collator Constructor": Intl.Collator.prototype,
  //    with its length 0, and Intl.Collator.prototype.constructor.
  defineConstructor(Collator, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(Collator, {
    // Intl.Collator.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be %Intl.Collator%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype.
  defineMethods(proto, {
    // Intl.Collator.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let collator be the this value.
      var collator = this;
      // 2. Perform ? RequireInternalSlot(collator, [[InitializedCollator]]).
      requireInitializedCollator(collator);
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. For each row of Table "Resolved Options of Collator Instances",
      //    except the header row, in table order, do
      //    a. Let propertyKey be the Property value of the current row.
      //    b. Let value be the value of collator's internal slot whose name
      //       is the Internal Slot value of the current row.
      //    c. If the current row has an Extension Key value, then
      //       i. Let extensionKey be the Extension Key value of the current
      //          row.
      //       ii. If %Intl.Collator%.[[RelevantExtensionKeys]] does not
      //           contain extensionKey, then
      //           1. Set value to undefined.
      //       It contains "kn" and "kf", the keys of the [[Numeric]] and
      //       [[CaseFirst]] rows.
      //    d. If value is not undefined, then
      //       i. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
      //          value).
      //    No slot is undefined once the constructor has run.
      CreateDataPropertyOrThrow(
        options,
        'locale',
        CollatorSlots.get(collator, 'locale'),
      );
      CreateDataPropertyOrThrow(
        options,
        'usage',
        CollatorSlots.get(collator, 'usage'),
      );
      CreateDataPropertyOrThrow(
        options,
        'sensitivity',
        CollatorSlots.get(collator, 'sensitivity'),
      );
      CreateDataPropertyOrThrow(
        options,
        'ignorePunctuation',
        CollatorSlots.get(collator, 'ignorePunctuation'),
      );
      CreateDataPropertyOrThrow(
        options,
        'collation',
        CollatorSlots.get(collator, 'collation'),
      );
      CreateDataPropertyOrThrow(
        options,
        'numeric',
        CollatorSlots.get(collator, 'numeric'),
      );
      CreateDataPropertyOrThrow(
        options,
        'caseFirst',
        CollatorSlots.get(collator, 'caseFirst'),
      );
      // 5. Return options.
      return options;
    },
  });
  // 5. The accessor properties of the prototype, with no set accessor.
  defineGetters(proto, {
    // get Intl.Collator.prototype.compare
    get compare() {
      // 1. Let collator be the this value.
      var collator = this;
      // 2. Perform ? RequireInternalSlot(collator, [[InitializedCollator]]).
      requireInitializedCollator(collator);
      // 3. If collator.[[BoundCompare]] is undefined, then
      //    a-b. Set collator.[[BoundCompare]] to a new built-in function
      //         that compares with collator (makeBoundCompare).
      if (CollatorSlots.get(collator, 'boundCompare') === undefined)
        CollatorSlots.setBoundCompare(collator, makeBoundCompare(collator));
      // 4. Return collator.[[BoundCompare]].
      return CollatorSlots.get(collator, 'boundCompare');
    },
  });
  // 6. Intl.Collator.prototype [ %Symbol.toStringTag% ]: "Intl.Collator",
  //    { [[Writable]]: false, [[Enumerable]]: false, [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.Collator');
  // 7. "The Intl.Collator Constructor": the initial value of the "Collator"
  //    property of the Intl object. ECMA-262 18 gives such data properties
  //    the attributes { [[Writable]]: true, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'Collator', {
    __proto__: null,
    value: Collator,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
