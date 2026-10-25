/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "NumberFormat Objects": the Intl.NumberFormat constructor with
// ChainNumberFormat and SetNumberFormatUnitOptions, its supportedLocalesOf,
// the prototype's resolvedOptions, format, formatToParts, formatRange,
// formatRangeToParts and @@toStringTag, UnwrapNumberFormat and
// CurrencyDigits, following the draft https://tc39.es/ecma402/ dated
// October 24, 2026. 41-number-patterns.js turns a value into parts.

/// %Intl.NumberFormat.prototype%; set by installNumberFormat.
var NumberFormatPrototype;

/// The internal slots of an Intl.NumberFormat instance ("Properties of
/// Intl.NumberFormat Instances"), as private fields of the object passed to
/// the constructor. [[InitializedNumberFormat]] is the presence of
/// #locale. Two fields are not slots of the draft:
/// - #rounding, the null-prototype record that FormatNumericToString and
///   ComputeExponent read (07-number.js): the slots SetNumberFormatDigitOptions
///   sets, as lowerCamelCase fields, and notation;
/// - #data, the data the formatter formats with (decision 5), which
///   numberFormatData returns.
class NumberFormatSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[LocaleData]]
  #localeData;
  /// [[NumberingSystem]]
  #numberingSystem;
  /// [[Style]]
  #style;
  /// [[Unit]]
  #unit;
  /// [[UnitDisplay]]
  #unitDisplay;
  /// [[Currency]]
  #currency;
  /// [[CurrencyDisplay]]
  #currencyDisplay;
  /// [[CurrencySign]]
  #currencySign;
  /// [[MinimumIntegerDigits]]
  #minimumIntegerDigits;
  /// [[MinimumFractionDigits]]
  #minimumFractionDigits;
  /// [[MaximumFractionDigits]]
  #maximumFractionDigits;
  /// [[MinimumSignificantDigits]]
  #minimumSignificantDigits;
  /// [[MaximumSignificantDigits]]
  #maximumSignificantDigits;
  /// [[RoundingType]]
  #roundingType;
  /// [[Notation]]
  #notation;
  /// [[CompactDisplay]]
  #compactDisplay;
  /// [[UseGrouping]]
  #useGrouping;
  /// [[SignDisplay]]
  #signDisplay;
  /// [[RoundingIncrement]]
  #roundingIncrement;
  /// [[RoundingMode]]
  #roundingMode;
  /// [[ComputedRoundingPriority]]
  #computedRoundingPriority;
  /// [[TrailingZeroDisplay]]
  #trailingZeroDisplay;
  /// [[BoundFormat]]
  #boundFormat;
  /// The record of the digit options and notation, for rounding.
  #rounding;
  /// The formatter's data.
  #data;

  /// Attach the slots to o, from the null-prototype record slots whose
  /// fields are named after the slots (a field that is absent is an
  /// undefined slot), with the records rounding and data.
  /// [[BoundFormat]] starts undefined.
  constructor(o, slots, rounding, data) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = slots.locale;
    this.#localeData = slots.localeData;
    this.#numberingSystem = slots.numberingSystem;
    this.#style = slots.style;
    this.#unit = slots.unit;
    this.#unitDisplay = slots.unitDisplay;
    this.#currency = slots.currency;
    this.#currencyDisplay = slots.currencyDisplay;
    this.#currencySign = slots.currencySign;
    this.#minimumIntegerDigits = slots.minimumIntegerDigits;
    this.#minimumFractionDigits = slots.minimumFractionDigits;
    this.#maximumFractionDigits = slots.maximumFractionDigits;
    this.#minimumSignificantDigits = slots.minimumSignificantDigits;
    this.#maximumSignificantDigits = slots.maximumSignificantDigits;
    this.#roundingType = slots.roundingType;
    this.#notation = slots.notation;
    this.#compactDisplay = slots.compactDisplay;
    this.#useGrouping = slots.useGrouping;
    this.#signDisplay = slots.signDisplay;
    this.#roundingIncrement = slots.roundingIncrement;
    this.#roundingMode = slots.roundingMode;
    this.#computedRoundingPriority = slots.computedRoundingPriority;
    this.#trailingZeroDisplay = slots.trailingZeroDisplay;
    this.#boundFormat = undefined;
    // 3. Keep the rounding record and the data.
    this.#rounding = rounding;
    this.#data = data;
  }

  /// Whether o is an Object with an [[InitializedNumberFormat]] internal
  /// slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedNumberFormat]]). name is the slot's name in
  /// lowerCamelCase, or "rounding" or "data".
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedNumberFormat]]).
    requireInitializedNumberFormat(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'localeData':
        return o.#localeData;
      case 'numberingSystem':
        return o.#numberingSystem;
      case 'style':
        return o.#style;
      case 'unit':
        return o.#unit;
      case 'unitDisplay':
        return o.#unitDisplay;
      case 'currency':
        return o.#currency;
      case 'currencyDisplay':
        return o.#currencyDisplay;
      case 'currencySign':
        return o.#currencySign;
      case 'minimumIntegerDigits':
        return o.#minimumIntegerDigits;
      case 'minimumFractionDigits':
        return o.#minimumFractionDigits;
      case 'maximumFractionDigits':
        return o.#maximumFractionDigits;
      case 'minimumSignificantDigits':
        return o.#minimumSignificantDigits;
      case 'maximumSignificantDigits':
        return o.#maximumSignificantDigits;
      case 'roundingType':
        return o.#roundingType;
      case 'notation':
        return o.#notation;
      case 'compactDisplay':
        return o.#compactDisplay;
      case 'useGrouping':
        return o.#useGrouping;
      case 'signDisplay':
        return o.#signDisplay;
      case 'roundingIncrement':
        return o.#roundingIncrement;
      case 'roundingMode':
        return o.#roundingMode;
      case 'computedRoundingPriority':
        return o.#computedRoundingPriority;
      case 'trailingZeroDisplay':
        return o.#trailingZeroDisplay;
      case 'boundFormat':
        return o.#boundFormat;
      case 'rounding':
        return o.#rounding;
      case 'data':
        return o.#data;
    }
    throw new _TypeError('Intl internal error: no NumberFormat slot ' + name);
  }

  /// Set o.[[BoundFormat]] to f, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedNumberFormat]]).
  static setBoundFormat(o, f) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedNumberFormat]]).
    requireInitializedNumberFormat(o);
    // 2. Set o.[[BoundFormat]] to f.
    o.#boundFormat = f;
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedNumberFormat]]).
function requireInitializedNumberFormat(o) {
  // 1. If o is not an Object, or has no [[InitializedNumberFormat]]
  //    internal slot, throw a TypeError exception.
  if (!NumberFormatSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.NumberFormat object');
}

// ECMA-262 7.3.21 OrdinaryHasInstance ( C, O )
function OrdinaryHasInstance(C, O) {
  // 1. If IsCallable(C) is false, return false.
  if (typeof C !== 'function') return false;
  // 2. If C has a [[BoundTargetFunction]] internal slot, then ...
  //    The callers pass a constructor of the plugin, which is not a bound
  //    function.
  // 3. If O is not an Object, return false.
  if (!isObject(O)) return false;
  // 4. Let P be ? Get(C, "prototype"). The plugin's constructors have a
  //    non-writable, non-configurable data property "prototype", so this
  //    Get runs no user code.
  var P = C.prototype;
  // 5. If P is not an Object, throw a TypeError exception.
  if (!isObject(P))
    throw new _TypeError('Function has non-object prototype in instanceof');
  // 6. Repeat,
  for (;;) {
    // a. Set O to ? O.[[GetPrototypeOf]]().
    O = ObjectGetPrototypeOf(O);
    // b. If O is null, return false.
    if (O === null) return false;
    // c. If SameValue(P, O) is true, return true.
    if (P === O) return true;
  }
}

/// The numbering systems of numbering-systems.txt, in that file's order
/// (sorted); computed on first use.
var availableNumberingSystems;

/// The cache of %Intl.NumberFormat%.[[LocaleData]].[[<locale>]] records.
var numberFormatLocaleDataCache = newDataCache();

/// %Intl.NumberFormat%.[[LocaleData]].[[<locale>]] for an available
/// locale: the record { nu }, where nu is the locale's default numbering
/// system (the defaultNumberingSystem helper's, so that
/// Intl.Locale.prototype.getNumberingSystems agrees) followed by the other
/// numbering systems of numbering-systems.txt in that file's order
/// (decision 17). The patterns and symbols the draft also puts in
/// [[LocaleData]] are requested by numberFormatData, for the numbering
/// system the constructor resolves.
function numberFormatLocaleData(locale) {
  // 1. If the cache has the record of locale, return it.
  var data = dataCacheGet(numberFormatLocaleDataCache, locale);
  if (data !== undefined) return data;
  // 2. Let nu be « the default numbering system of locale ».
  var first = native.defaultNumberingSystem(locale);
  var nu = [first];
  // 3. Append every other numbering system, in order.
  if (availableNumberingSystems === undefined)
    availableNumberingSystems = splitSpaces(
      native.supportedValues('numberingSystem'),
    );
  for (var i = 0; i < availableNumberingSystems.length; i++) {
    if (availableNumberingSystems[i] !== first)
      listPush(nu, availableNumberingSystems[i]);
  }
  // 4. Cache and return the record.
  data = {__proto__: null, nu: nu};
  dataCachePut(numberFormatLocaleDataCache, locale, data);
  return data;
}

/// The fields of %Intl.NumberFormat% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list (decision 17),
/// [[RelevantExtensionKeys]] « "nu" », and [[ResolutionOptionDescriptors]].
var NUMBER_FORMAT_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: ['nu'],
  resolutionOptionDescriptors: [
    {__proto__: null, key: 'nu', property: 'numberingSystem'},
  ],
};

// 6.3.1 IsWellFormedCurrencyCode ( currency )
function IsWellFormedCurrencyCode(currency) {
  // 1. If the length of currency is not 3, return false.
  if (currency.length !== 3) return false;
  // 2. Let normalized be the ASCII-uppercase of currency.
  var normalized = asciiUppercase(currency);
  // 3. If normalized contains any code unit outside of 0x0041 through
  //    0x005A (corresponding to Unicode characters LATIN CAPITAL LETTER A
  //    through LATIN CAPITAL LETTER Z), return false.
  for (var i = 0; i < 3; i++) {
    var c = charCodeAt(normalized, i);
    if (c < 0x41 || c > 0x5a) return false;
  }
  // 4. Return true.
  return true;
}

// 6.6.2 IsSanctionedSingleUnitIdentifier ( unitIdentifier )
function IsSanctionedSingleUnitIdentifier(unitIdentifier) {
  // 1. If unitIdentifier is listed in Table 2 below, return true.
  // 2. Else, return false.
  return listContains(SANCTIONED_UNITS, unitIdentifier);
}

// 6.6.1 IsWellFormedUnitIdentifier ( unitIdentifier )
function IsWellFormedUnitIdentifier(unitIdentifier) {
  // 1. If IsSanctionedSingleUnitIdentifier(unitIdentifier) is true, then
  //    a. Return true.
  if (IsSanctionedSingleUnitIdentifier(unitIdentifier)) return true;
  // 2. Let numeratorAndDenominator be StringSplitToList(unitIdentifier,
  //    "-per-").
  var numeratorAndDenominator = StringSplitToList(unitIdentifier, '-per-');
  // 3. If numeratorAndDenominator does not have exactly 2 elements, return
  //    false.
  if (numeratorAndDenominator.length !== 2) return false;
  // 4. If IsSanctionedSingleUnitIdentifier(numeratorAndDenominator[0]) is
  //    false, return false.
  if (!IsSanctionedSingleUnitIdentifier(numeratorAndDenominator[0]))
    return false;
  // 5. If IsSanctionedSingleUnitIdentifier(numeratorAndDenominator[1]) is
  //    false, return false.
  if (!IsSanctionedSingleUnitIdentifier(numeratorAndDenominator[1]))
    return false;
  // 6. Return true.
  return true;
}

// CurrencyDigits ( currency )
function CurrencyDigits(currency) {
  // 1. Assert: IsWellFormedCurrencyCode(currency) is true.
  // 2. Return a non-negative integer indicating the number of fractional
  //    digits used when formatting quantities of the currency
  //    corresponding to currency. If there is no available information on
  //    the number of digits to be used, return 2. That is CLDR's digits
  //    for the currency (decision 8); currency is uppercase here, as the
  //    helper requires.
  return native.currencyDigits(currency);
}

// SetNumberFormatUnitOptions ( intlObj, options )
// intlObj is the null-prototype record of the slots.
function SetNumberFormatUnitOptions(intlObj, options) {
  // 1. Let style be ? GetOption(options, "style", string, « "decimal",
  //    "percent", "currency", "unit" », "decimal").
  var style = GetOption(
    options,
    'style',
    'string',
    ['decimal', 'percent', 'currency', 'unit'],
    'decimal',
  );
  // 2. Set intlObj.[[Style]] to style.
  intlObj.style = style;
  // 3. Let currency be ? GetOption(options, "currency", string, empty,
  //    undefined).
  var currency = GetOption(options, 'currency', 'string', undefined, undefined);
  if (currency === undefined) {
    // 4. If currency is undefined, then
    //    a. If style is "currency", throw a TypeError exception.
    if (style === 'currency')
      throw new _TypeError('Currency code is required with currency style');
  } else if (!IsWellFormedCurrencyCode(currency)) {
    // 5. Else,
    //    a. If IsWellFormedCurrencyCode(currency) is false, throw a
    //       RangeError exception.
    throw new _RangeError('Invalid currency code: ' + currency);
  }
  // 6. Let currencyDisplay be ? GetOption(options, "currencyDisplay",
  //    string, « "code", "symbol", "narrowSymbol", "name" », "symbol").
  var currencyDisplay = GetOption(
    options,
    'currencyDisplay',
    'string',
    ['code', 'symbol', 'narrowSymbol', 'name'],
    'symbol',
  );
  // 7. Let currencySign be ? GetOption(options, "currencySign", string,
  //    « "standard", "accounting" », "standard").
  var currencySign = GetOption(
    options,
    'currencySign',
    'string',
    ['standard', 'accounting'],
    'standard',
  );
  // 8. Let unit be ? GetOption(options, "unit", string, empty, undefined).
  var unit = GetOption(options, 'unit', 'string', undefined, undefined);
  if (unit === undefined) {
    // 9. If unit is undefined, then
    //    a. If style is "unit", throw a TypeError exception.
    if (style === 'unit')
      throw new _TypeError('Unit is required with unit style');
  } else if (!IsWellFormedUnitIdentifier(unit)) {
    // 10. Else,
    //     a. If IsWellFormedUnitIdentifier(unit) is false, throw a
    //        RangeError exception.
    throw new _RangeError('Invalid unit argument: ' + unit);
  }
  // 11. Let unitDisplay be ? GetOption(options, "unitDisplay", string,
  //     « "short", "narrow", "long" », "short").
  var unitDisplay = GetOption(
    options,
    'unitDisplay',
    'string',
    ['short', 'narrow', 'long'],
    'short',
  );
  // 12. If style is "currency", then
  if (style === 'currency') {
    // a. Set intlObj.[[Currency]] to the ASCII-uppercase of currency.
    intlObj.currency = asciiUppercase(currency);
    // b. Set intlObj.[[CurrencyDisplay]] to currencyDisplay.
    intlObj.currencyDisplay = currencyDisplay;
    // c. Set intlObj.[[CurrencySign]] to currencySign.
    intlObj.currencySign = currencySign;
  }
  // 13. If style is "unit", then
  if (style === 'unit') {
    // a. Set intlObj.[[Unit]] to unit.
    intlObj.unit = unit;
    // b. Set intlObj.[[UnitDisplay]] to unitDisplay.
    intlObj.unitDisplay = unitDisplay;
  }
  // 14. Return unused.
}

/// The record of the digit options and notation of the null-prototype
/// record slots, which FormatNumericToString and ComputeExponent read.
function numberFormatRounding(slots) {
  // 1. Return a copy of the fields SetNumberFormatDigitOptions set, and
  //    notation.
  return {
    __proto__: null,
    minimumIntegerDigits: slots.minimumIntegerDigits,
    minimumFractionDigits: slots.minimumFractionDigits,
    maximumFractionDigits: slots.maximumFractionDigits,
    minimumSignificantDigits: slots.minimumSignificantDigits,
    maximumSignificantDigits: slots.maximumSignificantDigits,
    roundingType: slots.roundingType,
    computedRoundingPriority: slots.computedRoundingPriority,
    roundingIncrement: slots.roundingIncrement,
    roundingMode: slots.roundingMode,
    trailingZeroDisplay: slots.trailingZeroDisplay,
    notation: slots.notation,
  };
}

/// Steps 3 to 26 of Intl.NumberFormat ( [ locales [ , options ] ] ), on the
/// object numberFormat that steps 1 and 2 made. The slots are collected in
/// a null-prototype record and attached at the end, with the formatter's
/// data, once every value is known; nothing between can observe them.
/// Returns numberFormat.
function initializeNumberFormat(numberFormat, locales, options) {
  // 3. Let optionsResolution be ? ResolveOptions(%Intl.NumberFormat%,
  //    %Intl.NumberFormat%.[[LocaleData]], locales, options,
  //    « coerce-options »).
  var optionsResolution = ResolveOptions(
    NUMBER_FORMAT_SERVICE,
    numberFormatLocaleData,
    locales,
    options,
    ['coerce-options'],
  );
  // 4. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 5. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  var slots = {__proto__: null};
  // 6. Set numberFormat.[[Locale]] to resolvedLocale.[[Locale]].
  slots.locale = resolvedLocale.locale;
  // 7. Set numberFormat.[[LocaleData]] to resolvedLocale.[[LocaleData]].
  slots.localeData = resolvedLocale.localeData;
  // 8. Set numberFormat.[[NumberingSystem]] to resolvedLocale.[[nu]].
  slots.numberingSystem = resolvedLocale.nu;
  // 9. Perform ? SetNumberFormatUnitOptions(numberFormat, options).
  SetNumberFormatUnitOptions(slots, options);
  // 10. Let style be numberFormat.[[Style]].
  var style = slots.style;
  // 11. Let notation be ? GetOption(options, "notation", string,
  //     « "standard", "scientific", "engineering", "compact" »,
  //     "standard").
  var notation = GetOption(
    options,
    'notation',
    'string',
    ['standard', 'scientific', 'engineering', 'compact'],
    'standard',
  );
  // 12. Set numberFormat.[[Notation]] to notation.
  slots.notation = notation;
  var minFDDefault, maxFDDefault;
  if (style === 'currency' && notation === 'standard') {
    // 13. If style is "currency" and notation is "standard", then
    //     a. Let currency be numberFormat.[[Currency]].
    //     b. Let cDigits be CurrencyDigits(currency).
    var cDigits = CurrencyDigits(slots.currency);
    //     c. Let minFDDefault be cDigits.
    minFDDefault = cDigits;
    //     d. Let maxFDDefault be cDigits.
    maxFDDefault = cDigits;
  } else if (style === 'percent') {
    // 14. Else if style is "percent", then
    //     a. Let minFDDefault be 0.
    minFDDefault = 0;
    //     b. Let maxFDDefault be 0.
    maxFDDefault = 0;
  } else {
    // 15. Else,
    //     a. Let minFDDefault be 0.
    minFDDefault = 0;
    //     b. Let maxFDDefault be 3.
    maxFDDefault = 3;
  }
  // 16. Perform ? SetNumberFormatDigitOptions(numberFormat, options,
  //     minFDDefault, maxFDDefault, notation).
  SetNumberFormatDigitOptions(
    slots,
    options,
    minFDDefault,
    maxFDDefault,
    notation,
  );
  // 17. Let compactDisplay be ? GetOption(options, "compactDisplay",
  //     string, « "short", "long" », "short").
  var compactDisplay = GetOption(
    options,
    'compactDisplay',
    'string',
    ['short', 'long'],
    'short',
  );
  // 18. Let defaultUseGrouping be "auto".
  var defaultUseGrouping = 'auto';
  // 19. If notation is "compact", then
  if (notation === 'compact') {
    // a. Set numberFormat.[[CompactDisplay]] to compactDisplay.
    slots.compactDisplay = compactDisplay;
    // b. Set defaultUseGrouping to "min2".
    defaultUseGrouping = 'min2';
  }
  // 20. NOTE: For historical reasons, the strings "true" and "false" are
  //     accepted and replaced with the default value.
  // 21. Let useGrouping be ? GetBooleanOrStringNumberFormatOption(options,
  //     "useGrouping", « "min2", "auto", "always", "true", "false" »,
  //     defaultUseGrouping).
  var useGrouping = GetBooleanOrStringNumberFormatOption(
    options,
    'useGrouping',
    ['min2', 'auto', 'always', 'true', 'false'],
    defaultUseGrouping,
  );
  // 22. If useGrouping is "true" or useGrouping is "false", set useGrouping
  //     to defaultUseGrouping.
  if (useGrouping === 'true' || useGrouping === 'false')
    useGrouping = defaultUseGrouping;
  // 23. If useGrouping is true, set useGrouping to "always".
  if (useGrouping === true) useGrouping = 'always';
  // 24. Set numberFormat.[[UseGrouping]] to useGrouping.
  slots.useGrouping = useGrouping;
  // 25. Let signDisplay be ? GetOption(options, "signDisplay", string,
  //     « "auto", "never", "always", "exceptZero", "negative" », "auto").
  var signDisplay = GetOption(
    options,
    'signDisplay',
    'string',
    ['auto', 'never', 'always', 'exceptZero', 'negative'],
    'auto',
  );
  // 26. Set numberFormat.[[SignDisplay]] to signDisplay.
  slots.signDisplay = signDisplay;
  //     Attach the slots, the rounding record and the formatter's data,
  //     which is requested here, once (decision 16).
  new NumberFormatSlots(
    numberFormat,
    slots,
    numberFormatRounding(slots),
    numberFormatData(slots),
  );
  //     Return numberFormat; the caller performs steps 27 and 28.
  return numberFormat;
}

// ChainNumberFormat ( numberFormat, newTarget, this )
function ChainNumberFormat(numberFormat, newTarget, thisValue) {
  // 1. If newTarget is undefined and ? OrdinaryHasInstance(
  //    %Intl.NumberFormat%, this) is true, then
  if (newTarget === undefined && OrdinaryHasInstance(NumberFormat, thisValue)) {
    // a. Perform ? DefinePropertyOrThrow(this, %Intl%.[[FallbackSymbol]],
    //    PropertyDescriptor { [[Value]]: numberFormat, [[Writable]]: false,
    //    [[Enumerable]]: false, [[Configurable]]: false }).
    //    The captured Object.defineProperty throws the TypeError when the
    //    definition fails.
    ObjectDefineProperty(thisValue, IntlFallbackSymbol, {
      __proto__: null,
      value: numberFormat,
      writable: false,
      enumerable: false,
      configurable: false,
    });
    // b. Return this.
    return thisValue;
  }
  // 2. Return numberFormat.
  return numberFormat;
}

// Intl.NumberFormat ( [ locales [ , options ] ] )
function NumberFormat(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, let newTarget be the active function
  //    object, else let newTarget be NewTarget.
  // 2. Let numberFormat be ? OrdinaryCreateFromConstructor(newTarget,
  //    "%Intl.NumberFormat.prototype%", « [[InitializedNumberFormat]], ...
  //    [[BoundFormat]] »). NumberFormatSlots has all of them.
  //    With new, Hermes created this from NewTarget before the call (design
  //    spec "Constructors"). Without new, newTarget is %Intl.NumberFormat%,
  //    whose "prototype" is the non-writable, non-configurable
  //    %Intl.NumberFormat.prototype%, so the object is made from that.
  //    initializeNumberFormat attaches the slots.
  var numberFormat =
    new.target === undefined ? ObjectCreate(NumberFormatPrototype) : this;
  // 3-26. The steps that resolve the slots, numbered there.
  initializeNumberFormat(numberFormat, locales, options);
  // 27. If the implementation supports the normative optional constructor
  //     mode of 4.3 Note 3, then
  //     a. Let this be the this value.
  //     b. Return ? ChainNumberFormat(numberFormat, NewTarget, this).
  //     The plugin supports it (design spec "Legacy constructor
  //     behavior"); step 28, "Return numberFormat", is not reached.
  return ChainNumberFormat(numberFormat, new.target, this);
}

/// The result of Construct(%Intl.NumberFormat%, « locales, options ») for
/// the plugin's own callers, such as the toLocaleString overrides (design
/// spec "Resistance to tampering"): every step of the constructor, with
/// newTarget %Intl.NumberFormat%, without going through any public
/// property. NewTarget is not undefined, so ChainNumberFormat returns the
/// new object.
function CreateNumberFormat(locales, options) {
  // 1-2. Let numberFormat be OrdinaryObjectCreate(
  //      %Intl.NumberFormat.prototype%).
  // 3-26. Initialize it.
  // 27-28. Return numberFormat.
  return initializeNumberFormat(
    ObjectCreate(NumberFormatPrototype),
    locales,
    options,
  );
}

// UnwrapNumberFormat ( nf )
function UnwrapNumberFormat(nf) {
  // 1. If nf is not an Object, throw a TypeError exception.
  if (!isObject(nf))
    throw new _TypeError('Receiver is not an Intl.NumberFormat object');
  // 2. If nf does not have an [[InitializedNumberFormat]] internal slot and
  //    ? OrdinaryHasInstance(%Intl.NumberFormat%, nf) is true, then
  //    a. Return ? Get(nf, %Intl%.[[FallbackSymbol]]).
  if (!NumberFormatSlots.is(nf) && OrdinaryHasInstance(NumberFormat, nf))
    return nf[IntlFallbackSymbol];
  // 3. Return nf.
  return nf;
}

/// The bound format function of nf: step 4 of get
/// Intl.NumberFormat.prototype.format. An arrow function returned from
/// here has the name "" and length 1, and is not a constructor.
function makeBoundFormat(nf) {
  // a. Let closure be a new Abstract Closure with parameters (value) that
  //    captures nf and performs the following steps when called:
  // b. Set nf.[[BoundFormat]] to CreateBuiltinFunction(closure, 1, "",
  //    « »). (The caller does this.)
  return value => {
    // i. Let x be ? ToIntlMathematicalValue(value).
    var x = ToIntlMathematicalValue(value);
    // ii. Return FormatNumeric(nf, x).
    return FormatNumeric(nf, x);
  };
}

/// Define obj[key] = the slot slotName of nf, if that slot is not
/// undefined: steps 4.a to 4.d of resolvedOptions, for one row of the table
/// "Resolved Options of NumberFormat Instances". Every slot with the
/// Conversion number already holds a Number.
function resolvedOption(options, nf, propertyKey, slotName) {
  // a. Let propertyKey be the Property value of the current row.
  // b. Let value be the value of nf's internal slot whose name is the
  //    Internal Slot value of the current row.
  var value = NumberFormatSlots.get(nf, slotName);
  // c. If value is not undefined, then
  if (value !== undefined) {
    // i. If there is a Conversion value in the current row, then
    //    1. Assert: The Conversion value of the current row is number.
    //    2. Set value to 𝔽(value).
    // ii. Perform ! CreateDataPropertyOrThrow(options, propertyKey, value).
    CreateDataPropertyOrThrow(options, propertyKey, value);
  }
}

/// The rows of the table "Resolved Options of NumberFormat Instances", in
/// table order: [Property, slot name].
var NUMBER_FORMAT_RESOLVED_OPTIONS = [
  ['locale', 'locale'],
  ['numberingSystem', 'numberingSystem'],
  ['style', 'style'],
  ['currency', 'currency'],
  ['currencyDisplay', 'currencyDisplay'],
  ['currencySign', 'currencySign'],
  ['unit', 'unit'],
  ['unitDisplay', 'unitDisplay'],
  ['minimumIntegerDigits', 'minimumIntegerDigits'],
  ['minimumFractionDigits', 'minimumFractionDigits'],
  ['maximumFractionDigits', 'maximumFractionDigits'],
  ['minimumSignificantDigits', 'minimumSignificantDigits'],
  ['maximumSignificantDigits', 'maximumSignificantDigits'],
  ['useGrouping', 'useGrouping'],
  ['notation', 'notation'],
  ['compactDisplay', 'compactDisplay'],
  ['signDisplay', 'signDisplay'],
  ['roundingIncrement', 'roundingIncrement'],
  ['roundingMode', 'roundingMode'],
  ['roundingPriority', 'computedRoundingPriority'],
  ['trailingZeroDisplay', 'trailingZeroDisplay'],
];

/// Install Intl.NumberFormat on intl; called by installIntl.
function installNumberFormat(intl) {
  // 1. Let proto be %Intl.NumberFormat.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.NumberFormat Prototype Object").
  var proto = {};
  NumberFormatPrototype = proto;
  // 2. "Properties of the Intl.NumberFormat Constructor":
  //    Intl.NumberFormat.prototype, with its length 0, and
  //    Intl.NumberFormat.prototype.constructor.
  defineConstructor(NumberFormat, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(NumberFormat, {
    // Intl.NumberFormat.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be %Intl.NumberFormat%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.NumberFormat.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let nf be the this value.
      // 2. If the implementation supports the normative optional
      //    constructor mode of 4.3 Note 3, then
      //    a. Set nf to ? UnwrapNumberFormat(nf).
      var nf = UnwrapNumberFormat(this);
      // 3. Perform ? RequireInternalSlot(nf, [[InitializedNumberFormat]]).
      requireInitializedNumberFormat(nf);
      // 4. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 5. For each row of Table "Resolved Options of NumberFormat
      //    Instances", except the header row, in table order, do
      //    a-d. (resolvedOption)
      for (var i = 0; i < NUMBER_FORMAT_RESOLVED_OPTIONS.length; i++) {
        var row = NUMBER_FORMAT_RESOLVED_OPTIONS[i];
        resolvedOption(options, nf, row[0], row[1]);
      }
      // 6. Return options.
      return options;
    },
  });
  defineGetters(proto, {
    // get Intl.NumberFormat.prototype.format
    get format() {
      // 1. Let nf be the this value.
      // 2. If the implementation supports the normative optional
      //    constructor mode of 4.3 Note 3, then
      //    a. Set nf to ? UnwrapNumberFormat(nf).
      var nf = UnwrapNumberFormat(this);
      // 3. Perform ? RequireInternalSlot(nf, [[InitializedNumberFormat]]).
      requireInitializedNumberFormat(nf);
      // 4. If nf.[[BoundFormat]] is undefined, then
      //    a-b. Set nf.[[BoundFormat]] to a new built-in function that
      //         formats with nf (makeBoundFormat).
      if (NumberFormatSlots.get(nf, 'boundFormat') === undefined)
        NumberFormatSlots.setBoundFormat(nf, makeBoundFormat(nf));
      // 5. Return nf.[[BoundFormat]].
      return NumberFormatSlots.get(nf, 'boundFormat');
    },
  });
  defineMethods(proto, {
    // Intl.NumberFormat.prototype.formatRange ( start, end )
    formatRange(start, end) {
      // 1. Let nf be the this value.
      var nf = this;
      // 2. Perform ? RequireInternalSlot(nf, [[InitializedNumberFormat]]).
      requireInitializedNumberFormat(nf);
      // 3. If start is undefined or end is undefined, throw a TypeError
      //    exception.
      if (start === undefined || end === undefined)
        throw new _TypeError('start and end are required');
      // 4. Let x be ? ToIntlMathematicalValue(start).
      var x = ToIntlMathematicalValue(start);
      // 5. Let y be ? ToIntlMathematicalValue(end).
      var y = ToIntlMathematicalValue(end);
      // 6. Return ? FormatNumericRange(nf, x, y).
      return FormatNumericRange(nf, x, y);
    },
    // Intl.NumberFormat.prototype.formatRangeToParts ( start, end )
    formatRangeToParts(start, end) {
      // 1. Let nf be the this value.
      var nf = this;
      // 2. Perform ? RequireInternalSlot(nf, [[InitializedNumberFormat]]).
      requireInitializedNumberFormat(nf);
      // 3. If start is undefined or end is undefined, throw a TypeError
      //    exception.
      if (start === undefined || end === undefined)
        throw new _TypeError('start and end are required');
      // 4. Let x be ? ToIntlMathematicalValue(start).
      var x = ToIntlMathematicalValue(start);
      // 5. Let y be ? ToIntlMathematicalValue(end).
      var y = ToIntlMathematicalValue(end);
      // 6. Return ? FormatNumericRangeToParts(nf, x, y).
      return FormatNumericRangeToParts(nf, x, y);
    },
    // Intl.NumberFormat.prototype.formatToParts ( value )
    formatToParts(value) {
      // 1. Let nf be the this value.
      var nf = this;
      // 2. Perform ? RequireInternalSlot(nf, [[InitializedNumberFormat]]).
      requireInitializedNumberFormat(nf);
      // 3. Let x be ? ToIntlMathematicalValue(value).
      var x = ToIntlMathematicalValue(value);
      // 4. Return FormatNumericToParts(nf, x).
      return FormatNumericToParts(nf, x);
    },
  });
  // 5. Intl.NumberFormat.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.NumberFormat", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.NumberFormat');
  // 6. "The Intl.NumberFormat Constructor": the initial value of the
  //    "NumberFormat" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'NumberFormat', {
    __proto__: null,
    value: NumberFormat,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
