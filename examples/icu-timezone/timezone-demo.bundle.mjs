import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

/**
 * Standalone WASM loader for bundled output.
 * Reads icu_capi.wasm from the same directory as the running script.
 */

function readString8FromWasm(wasmExports, ptr, len) {
  const buf = new Uint8Array(wasmExports.memory.buffer, ptr, len);
  // Use a manual decoder since TextDecoder may not be available everywhere
  let str = '';
  for (let i = 0; i < buf.length; i++) {
    str += String.fromCharCode(buf[i]);
  }
  return str;
}

let wasm;

const imports = {
  env: {
    diplomat_console_debug_js(_ptr, _len) {},
    diplomat_console_error_js(_ptr, _len) {},
    diplomat_console_info_js(_ptr, _len) {},
    diplomat_console_log_js(_ptr, _len) {},
    diplomat_console_warn_js(_ptr, _len) {},
    diplomat_throw_error_js(ptr, len) {
      throw new Error(readString8FromWasm(wasm, ptr, len));
    },
  },
};

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const wasmPath = join(__dirname, 'icu_capi.wasm');
const wasmBytes = readFileSync(wasmPath);
const wasmModule = new WebAssembly.Module(wasmBytes);
const wasmInstance = new WebAssembly.Instance(wasmModule, imports);
wasm = wasmInstance.exports;
wasm.diplomat_init();

var wasm$1 = wasm;

/** For internal Diplomat use when constructing opaques or out structs.
 * This is for when we're handling items that we don't want the user to touch, like an structure that's only meant to be output, or de-referencing a pointer we're handed from WASM.
 */
const internalConstructor = Symbol("constructor");
/** For internal Diplomat use when accessing a from-fields/from-value constructor that's been overridden by a default constructor.
 * If we want to pass in arguments without also passing in internalConstructor to avoid triggering some logic we don't want, we use exposeConstructor.
 */
const exposeConstructor = Symbol("exposeConstructor");

function readString8(wasm, ptr, len) {
    const buf = new Uint8Array(wasm.memory.buffer, ptr, len);
    return (new TextDecoder("utf-8")).decode(buf)
}

/**
 * Get the pointer returned by an FFI function.
 *
 * It's tempting to call `(new Uint32Array(wasm.memory.buffer, FFI_func(), 1))[0]`.
 * However, there's a chance that `wasm.memory.buffer` will be resized between
 * the time it's accessed and the time it's used, invalidating the view.
 * This function ensures that the view into wasm memory is fresh.
 *
 * This is used for methods that return multiple types into a wasm buffer, where
 * one of those types is another ptr. Call this method to get access to the returned
 * ptr, so the return buffer can be freed.
 * @param {WebAssembly.Exports} wasm Provided by diplomat generated files.
 * @param {number} ptr Pointer of a pointer, to be read.
 * @returns {number} The underlying pointer.
 */
function ptrRead(wasm, ptr) {
    return (new Uint32Array(wasm.memory.buffer, ptr, 1))[0];
}

/**
 * Get the flag of a result type.
 */
function resultFlag(wasm, ptr, offset) {
    return (new Uint8Array(wasm.memory.buffer, ptr + offset, 1))[0];
}

/**
 * Get the discriminant of a Rust enum.
*/
function enumDiscriminant(wasm, ptr) {
    return (new Int32Array(wasm.memory.buffer, ptr, 1))[0]
}

/**
* Write a value of width `width` to a an ArrayBuffer `arrayBuffer`
* at byte offset `offset`, treating it as a buffer of kind `typedArrayKind`
* (which is a `TypedArray` variant like `Uint8Array` or `Int16Array`)
*/
function writeToArrayBuffer(arrayBuffer, offset, value, typedArrayKind) {
    let buffer = new typedArrayKind(arrayBuffer, offset);
    buffer[0] = value;
}

/**
* Take `jsValue` and write it to arrayBuffer at offset `offset` if it is non-null
* calling `writeToArrayBufferCallback(arrayBuffer, offset, jsValue)` to write to the buffer,
* also writing a tag bit.
*
* `size` and `align` are the size and alignment of T, not of Option<T>
*/
function writeOptionToArrayBuffer(arrayBuffer, offset, jsValue, size, align, writeToArrayBufferCallback) {
    // perform a nullish check, not a null check,
    // we want identical behavior for undefined
    if (jsValue != null) {
        writeToArrayBufferCallback(arrayBuffer, offset, jsValue);
        writeToArrayBuffer(arrayBuffer, offset + size, 1, Uint8Array);
    }
}

/**
* For Option<T> of given size/align (of T, not the overall option type),
* return a pointer to wasm memory, allocated in `allocator`, that stores a `jsValue`
* of that option type (or `null`).
*
* Calls writeToArrayBufferCallback(arrayBuffer, offset, jsValue) for non-null jsValues.
*
* This array will have size<T>/align<T> elements for the actual T, then one element
* for the is_ok bool.
*/
function optionToBufferForCalling(wasm, jsValue, size, align, allocator, writeToArrayBufferCallback) {
    let buf = DiplomatBuf.struct(wasm, size + align, align);

    let buffer;
    // Add 1 to the size since we're also accounting for the 0 or 1 is_ok field:
    if (align == 8) {
        buffer = new BigUint64Array(wasm.memory.buffer, buf.ptr, size / align + 1);
    } else if (align == 4) {
        buffer = new Uint32Array(wasm.memory.buffer, buf.ptr, size / align + 1);
    } else if (align == 2) {
        buffer = new Uint16Array(wasm.memory.buffer, buf.ptr, size / align + 1);
    } else {
        buffer = new Uint8Array(wasm.memory.buffer, buf.ptr, size / align + 1);
    }

    buffer.fill(0);

    if (jsValue != null) {
        // Note that `buffer.buffer` is the underlying ArrayBuffer (`buffer` is just a view),
        // so we must provide the offset pointer (buf.ptr)
        writeToArrayBufferCallback(buffer.buffer, buf.ptr, jsValue);
        buffer[buffer.length - 1] = 1;
    }

    return allocator.alloc(buf).ptr;
}


/**
* Given `ptr` in Wasm memory, treat it as an Option<T> with size for type T,
* and return the converted T (converted using `readCallback(wasm, ptr)`) if the Option is Some
* else None.
*/
function readOption(wasm, ptr, size, readCallback) {
    // Don't need the alignment: diplomat types don't have overridden alignment,
    // so the flag will immediately be after the inner struct.
    let flag = resultFlag(wasm, ptr, size);
    if (flag) {
        return readCallback(wasm, ptr);
    } else {
        return null;
    }
}

/**
 * A wrapper around a slice of WASM memory that can be freed manually or
 * automatically by the garbage collector.
 *
 * This type is necessary for Rust functions that take a `&str` or `&[T]`, since
 * they can create an edge to this object if they borrow from the str/slice,
 * or we can manually free the WASM memory if they don't.
 */
class DiplomatBuf {
    static str8 = (wasm, string) => {
    var utf8Length = 0;
    for (const codepointString of string) {
        let codepoint = codepointString.codePointAt(0);
        if (codepoint < 0x80) {
        utf8Length += 1;
        } else if (codepoint < 0x800) {
        utf8Length += 2;
        } else if (codepoint < 0x10000) {
        utf8Length += 3;
        } else {
        utf8Length += 4;
        }
    }

    const ptr = wasm.diplomat_alloc(utf8Length, 1);

    const result = (new TextEncoder()).encodeInto(string, new Uint8Array(wasm.memory.buffer, ptr, utf8Length));
    console.assert(string.length === result.read && utf8Length === result.written, "UTF-8 write error");

    return new DiplomatBuf(ptr, utf8Length, () => wasm.diplomat_free(ptr, utf8Length, 1));
    }

    static str16 = (wasm, string) => {
    const byteLength = string.length * 2;
    const ptr = wasm.diplomat_alloc(byteLength, 2);

    const destination = new Uint16Array(wasm.memory.buffer, ptr, string.length);
    for (let i = 0; i < string.length; i++) {
        destination[i] = string.charCodeAt(i);
    }

    return new DiplomatBuf(ptr, string.length, () => wasm.diplomat_free(ptr, byteLength, 2));
    }

    static sliceWrapper = (wasm, buf) => {
        const ptr = wasm.diplomat_alloc(8, 4);
        let dst = new Uint32Array(wasm.memory.buffer, ptr, 2);

        dst[0] = buf.ptr;
        dst[1] = buf.size;
        return new DiplomatBuf(ptr, 8, () => {
            wasm.diplomat_free(ptr, 8, 4);
            buf.free();
        });
    }

    static slice = (wasm, list, rustType) => {
    const elementSize = rustType === "u8" || rustType === "i8" || rustType === "boolean" ? 1 :
        rustType === "u16" || rustType === "i16" ? 2 :
        rustType === "u64" || rustType === "i64" || rustType === "f64" ? 8 :
            4;

    const byteLength = list.length * elementSize;
    const ptr = wasm.diplomat_alloc(byteLength, elementSize);

    /**
     * Create an array view of the buffer. This gives us the `set` method which correctly handles untyped values
     */
    const destination =
        rustType === "u8" || rustType === "boolean" ? new Uint8Array(wasm.memory.buffer, ptr, byteLength) :
        rustType === "i8" ? new Int8Array(wasm.memory.buffer, ptr, byteLength) :
            rustType === "u16" ? new Uint16Array(wasm.memory.buffer, ptr, byteLength) :
            rustType === "i16" ? new Int16Array(wasm.memory.buffer, ptr, byteLength) :
                rustType === "i32" ? new Int32Array(wasm.memory.buffer, ptr, byteLength) :
                rustType === "u64" ? new BigUint64Array(wasm.memory.buffer, ptr, byteLength) :
                    rustType === "i64" ? new BigInt64Array(wasm.memory.buffer, ptr, byteLength) :
                    rustType === "f32" ? new Float32Array(wasm.memory.buffer, ptr, byteLength) :
                        rustType === "f64" ? new Float64Array(wasm.memory.buffer, ptr, byteLength) :
                        new Uint32Array(wasm.memory.buffer, ptr, byteLength);
    destination.set(list);

    return new DiplomatBuf(ptr, list.length, () => wasm.diplomat_free(ptr, byteLength, elementSize));
    }

    static strs = (wasm, strings, encoding) => {
        let encodeStr = (encoding === "string16") ? DiplomatBuf.str16 : DiplomatBuf.str8;

        const byteLength = strings.length * 4 * 2;

        const ptr = wasm.diplomat_alloc(byteLength, 4);

        const destination = new Uint32Array(wasm.memory.buffer, ptr, byteLength);

        const stringsAlloc = [];

        for (let i = 0; i < strings.length; i++) {
            stringsAlloc.push(encodeStr(wasm, strings[i]));

            destination[2 * i] = stringsAlloc[i].ptr;
            destination[(2 * i) + 1] = stringsAlloc[i].size;
        }

        return new DiplomatBuf(ptr, strings.length, () => {
            wasm.diplomat_free(ptr, byteLength, 4);
            for (let i = 0; i < stringsAlloc.length; i++) {
                stringsAlloc[i].free();
            }
        });
    }

    static struct = (wasm, size, align) => {
        const ptr = wasm.diplomat_alloc(size, align);

        return new DiplomatBuf(ptr, size, () => {
            wasm.diplomat_free(ptr, size, align);
        });
    }

    /**
     * Generated code calls one of methods these for each allocation, to either
     * free directly after the FFI call, to leak (to create a &'static), or to
     * register the buffer with the garbage collector (to create a &'a).
     */
    free;

    constructor(ptr, size, free) {
        this.ptr = ptr;
        this.size = size;
        this.free = free;
        this.leak = () => { };
        this.releaseToGarbageCollector = () => DiplomatBufferFinalizer.register(this, () => this.free());
    }

    splat() {
        return [this.ptr, this.size];
    }

    /**
     * Write the (ptr, len) pair to an array buffer at byte offset `offset`
     */
    writePtrLenToArrayBuffer(arrayBuffer, offset) {
        writeToArrayBuffer(arrayBuffer, offset, this.ptr, Uint32Array);
        writeToArrayBuffer(arrayBuffer, offset + 4, this.size, Uint32Array);
    }
}

/**
 * Helper class for creating and managing `diplomat_buffer_write`.
 * Meant to minimize direct calls to `wasm`.
 */
class DiplomatWriteBuf {
    leak;

    #wasm;
    #buffer;

    constructor(wasm) {
        this.#wasm = wasm;
        this.#buffer = this.#wasm.diplomat_buffer_write_create(0);

        this.leak = () => { };
    }

    free() {
        this.#wasm.diplomat_buffer_write_destroy(this.#buffer);
    }

    releaseToGarbageCollector() {
        DiplomatBufferFinalizer.register(this, () => this.free());
    }

    readString8() {
        return readString8(this.#wasm, this.ptr, this.size);
    }

    get buffer() {
        return this.#buffer;
    }

    get ptr() {
        return this.#wasm.diplomat_buffer_write_get_bytes(this.#buffer);
    }

    get size() {
        return this.#wasm.diplomat_buffer_write_len(this.#buffer);
    }
}

/**
 * Represents an underlying slice that we've grabbed from WebAssembly.
 * You can treat this in JS as a regular slice of primitives, but it handles additional data for you behind the scenes.
 */
class DiplomatSlice {
    #wasm;

    #bufferType;
    get bufferType() {
        return this.#bufferType;
    }

    #buffer;
    get buffer() {
        return this.#buffer;
    }

    #lifetimeEdges;

    constructor(wasm, buffer, bufferType, lifetimeEdges) {
        this.#wasm = wasm;

        const [ptr, size] = new Uint32Array(this.#wasm.memory.buffer, buffer, 2);

        this.#buffer = new bufferType(this.#wasm.memory.buffer, ptr, size);
        this.#bufferType = bufferType;

        this.#lifetimeEdges = lifetimeEdges;
    }

    getValue() {
        return this.#buffer;
    }

    [Symbol.toPrimitive]() {
        return this.getValue();
    }

    valueOf() {
        return this.getValue();
    }
}

class DiplomatSliceStr extends DiplomatSlice {
    #decoder;

    constructor(wasm, buffer, stringEncoding, lifetimeEdges) {
        let encoding;
        switch (stringEncoding) {
            case "string8":
                encoding = Uint8Array;
                break;
            case "string16":
                encoding = Uint16Array;
                break;
            default:
                console.error("Unrecognized stringEncoding ", stringEncoding);
                break;
        }
        super(wasm, buffer, encoding, lifetimeEdges);

        if (stringEncoding === "string8") {
            this.#decoder = new TextDecoder('utf-8');
        }
    }

    getValue() {
        switch (this.bufferType) {
            case Uint8Array:
                return this.#decoder.decode(super.getValue());
            case Uint16Array:
                return String.fromCharCode.apply(null, super.getValue());
            default:
                return null;
        }
    }

    toString() {
        return this.getValue();
    }
}

/**
 * A number of Rust functions in WebAssembly require a buffer to populate struct, slice, Option<> or Result<> types with information.
 * {@link DiplomatReceiveBuf} allocates a buffer in WebAssembly, which can then be passed into functions with the {@link DiplomatReceiveBuf.buffer}
 * property.
 */
class DiplomatReceiveBuf {
    #wasm;

    #size;
    #align;

    #hasResult;

    #buffer;

    constructor(wasm, size, align, hasResult) {
        this.#wasm = wasm;

        this.#size = size;
        this.#align = align;

        this.#hasResult = hasResult;

        this.#buffer = this.#wasm.diplomat_alloc(this.#size, this.#align);

        this.leak = () => { };
    }

    free() {
        this.#wasm.diplomat_free(this.#buffer, this.#size, this.#align);
    }

    get buffer() {
        return this.#buffer;
    }

    /**
     * Only for when a DiplomatReceiveBuf is allocating a buffer for an `Option<>` or a `Result<>` type.
     *
     * This just checks the last byte for a successful result (assuming that Rust's compiler does not change).
     */
    get resultFlag() {
        if (this.#hasResult) {
            return resultFlag(this.#wasm, this.#buffer, this.#size - 1);
        } else {
            return true;
        }
    }
}

/**
 * For cleaning up slices inside struct _intoFFI functions.
 * Based somewhat on how the Dart backend handles slice cleanup.
 *
 * We want to ensure a slice only lasts as long as its struct, so we have a `functionCleanupArena` CleanupArena that we use in each method for any slice that needs to be cleaned up. It lasts only as long as the function is called for.
 *
 * Then we have `createWith`, which is meant for longer lasting slices. It takes an array of edges and will last as long as those edges do. Cleanup is only called later.
 */
class CleanupArena {
    #items = [];

    constructor() {
    }

    /**
     * When this arena is freed, call .free() on the given item.
     * @param {DiplomatBuf} item
     * @returns {DiplomatBuf}
     */
    alloc(item) {
        this.#items.push(item);
        return item;
    }
    /**
     * Create a new CleanupArena, append it to any edge arrays passed down, and return it.
     * @param {Array} edgeArrays
     * @returns {CleanupArena}
     */
    static createWith(...edgeArrays) {
        let self = new CleanupArena();
        for (let edgeArray of edgeArrays) {
            if (edgeArray != null) {
                edgeArray.push(self);
            }
        }
        DiplomatBufferFinalizer.register(self, () => self.free());
        return self;
    }

    /**
     * If given edge arrays, create a new CleanupArena, append it to any edge arrays passed down, and return it.
     * Else return the function-local cleanup arena
     * @param {CleanupArena} functionCleanupArena
     * @param {Array} edgeArrays
     * @returns {DiplomatBuf}
     */
    static maybeCreateWith(functionCleanupArena, ...edgeArrays) {
        if (edgeArrays.length > 0) {
            return CleanupArena.createWith(...edgeArrays);
        } else {
            return functionCleanupArena
        }
    }

    free() {
        this.#items.forEach((i) => {
            i.free();
        });

        this.#items.length = 0;
    }
}

const DiplomatBufferFinalizer = new FinalizationRegistry(free => free());

// generated by diplomat-tool



/**
 * See the [Rust documentation for `LeadingAdjustment`](https://docs.rs/icu/2.1.1/icu/casemap/options/enum.LeadingAdjustment.html) for more information.
 */
class LeadingAdjustment {
    #value = undefined;

    static #values = new Map([
        ["Auto", 0],
        ["None", 1],
        ["ToCased", 2]
    ]);

    static getAllEntries() {
        return LeadingAdjustment.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LeadingAdjustment.#objectValues[arguments[1]];
        }

        if (value instanceof LeadingAdjustment) {
            return value;
        }

        let intVal = LeadingAdjustment.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LeadingAdjustment.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LeadingAdjustment and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LeadingAdjustment(value);
    }

    get value(){
        return [...LeadingAdjustment.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LeadingAdjustment(internalConstructor, internalConstructor, 0),
        new LeadingAdjustment(internalConstructor, internalConstructor, 1),
        new LeadingAdjustment(internalConstructor, internalConstructor, 2),
    ];

    static Auto = LeadingAdjustment.#objectValues[0];
    static None = LeadingAdjustment.#objectValues[1];
    static ToCased = LeadingAdjustment.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `TrailingCase`](https://docs.rs/icu/2.1.1/icu/casemap/options/enum.TrailingCase.html) for more information.
 */
class TrailingCase {
    #value = undefined;

    static #values = new Map([
        ["Lower", 0],
        ["Unchanged", 1]
    ]);

    static getAllEntries() {
        return TrailingCase.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return TrailingCase.#objectValues[arguments[1]];
        }

        if (value instanceof TrailingCase) {
            return value;
        }

        let intVal = TrailingCase.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return TrailingCase.#objectValues[intVal];
        }

        throw TypeError(value + " is not a TrailingCase and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new TrailingCase(value);
    }

    get value(){
        return [...TrailingCase.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new TrailingCase(internalConstructor, internalConstructor, 0),
        new TrailingCase(internalConstructor, internalConstructor, 1),
    ];

    static Lower = TrailingCase.#objectValues[0];
    static Unchanged = TrailingCase.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `AlternateHandling`](https://docs.rs/icu/2.1.1/icu/collator/options/enum.AlternateHandling.html) for more information.
 */
class CollatorAlternateHandling {
    #value = undefined;

    static #values = new Map([
        ["NonIgnorable", 0],
        ["Shifted", 1]
    ]);

    static getAllEntries() {
        return CollatorAlternateHandling.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CollatorAlternateHandling.#objectValues[arguments[1]];
        }

        if (value instanceof CollatorAlternateHandling) {
            return value;
        }

        let intVal = CollatorAlternateHandling.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CollatorAlternateHandling.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CollatorAlternateHandling and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CollatorAlternateHandling(value);
    }

    get value(){
        return [...CollatorAlternateHandling.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CollatorAlternateHandling(internalConstructor, internalConstructor, 0),
        new CollatorAlternateHandling(internalConstructor, internalConstructor, 1),
    ];

    static NonIgnorable = CollatorAlternateHandling.#objectValues[0];
    static Shifted = CollatorAlternateHandling.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `CaseLevel`](https://docs.rs/icu/2.1.1/icu/collator/options/enum.CaseLevel.html) for more information.
 */
class CollatorCaseLevel {
    #value = undefined;

    static #values = new Map([
        ["Off", 0],
        ["On", 1]
    ]);

    static getAllEntries() {
        return CollatorCaseLevel.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CollatorCaseLevel.#objectValues[arguments[1]];
        }

        if (value instanceof CollatorCaseLevel) {
            return value;
        }

        let intVal = CollatorCaseLevel.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CollatorCaseLevel.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CollatorCaseLevel and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CollatorCaseLevel(value);
    }

    get value(){
        return [...CollatorCaseLevel.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CollatorCaseLevel(internalConstructor, internalConstructor, 0),
        new CollatorCaseLevel(internalConstructor, internalConstructor, 1),
    ];

    static Off = CollatorCaseLevel.#objectValues[0];
    static On = CollatorCaseLevel.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `MaxVariable`](https://docs.rs/icu/2.1.1/icu/collator/options/enum.MaxVariable.html) for more information.
 */
class CollatorMaxVariable {
    #value = undefined;

    static #values = new Map([
        ["Space", 0],
        ["Punctuation", 1],
        ["Symbol", 2],
        ["Currency", 3]
    ]);

    static getAllEntries() {
        return CollatorMaxVariable.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CollatorMaxVariable.#objectValues[arguments[1]];
        }

        if (value instanceof CollatorMaxVariable) {
            return value;
        }

        let intVal = CollatorMaxVariable.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CollatorMaxVariable.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CollatorMaxVariable and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CollatorMaxVariable(value);
    }

    get value(){
        return [...CollatorMaxVariable.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CollatorMaxVariable(internalConstructor, internalConstructor, 0),
        new CollatorMaxVariable(internalConstructor, internalConstructor, 1),
        new CollatorMaxVariable(internalConstructor, internalConstructor, 2),
        new CollatorMaxVariable(internalConstructor, internalConstructor, 3),
    ];

    static Space = CollatorMaxVariable.#objectValues[0];
    static Punctuation = CollatorMaxVariable.#objectValues[1];
    static Symbol = CollatorMaxVariable.#objectValues[2];
    static Currency = CollatorMaxVariable.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Strength`](https://docs.rs/icu/2.1.1/icu/collator/options/enum.Strength.html) for more information.
 */
class CollatorStrength {
    #value = undefined;

    static #values = new Map([
        ["Primary", 0],
        ["Secondary", 1],
        ["Tertiary", 2],
        ["Quaternary", 3],
        ["Identical", 4]
    ]);

    static getAllEntries() {
        return CollatorStrength.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CollatorStrength.#objectValues[arguments[1]];
        }

        if (value instanceof CollatorStrength) {
            return value;
        }

        let intVal = CollatorStrength.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CollatorStrength.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CollatorStrength and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CollatorStrength(value);
    }

    get value(){
        return [...CollatorStrength.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CollatorStrength(internalConstructor, internalConstructor, 0),
        new CollatorStrength(internalConstructor, internalConstructor, 1),
        new CollatorStrength(internalConstructor, internalConstructor, 2),
        new CollatorStrength(internalConstructor, internalConstructor, 3),
        new CollatorStrength(internalConstructor, internalConstructor, 4),
    ];

    static Primary = CollatorStrength.#objectValues[0];
    static Secondary = CollatorStrength.#objectValues[1];
    static Tertiary = CollatorStrength.#objectValues[2];
    static Quaternary = CollatorStrength.#objectValues[3];
    static Identical = CollatorStrength.#objectValues[4];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `DateFields`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.DateFields.html) for more information.
 */
class DateFields {
    #era;
    get era() {
        return this.#era;
    }
    set era(value){
        this.#era = value;
    }
    #eraYear;
    get eraYear() {
        return this.#eraYear;
    }
    set eraYear(value){
        this.#eraYear = value;
    }
    #extendedYear;
    get extendedYear() {
        return this.#extendedYear;
    }
    set extendedYear(value){
        this.#extendedYear = value;
    }
    #monthCode;
    get monthCode() {
        return this.#monthCode;
    }
    set monthCode(value){
        this.#monthCode = value;
    }
    #ordinalMonth;
    get ordinalMonth() {
        return this.#ordinalMonth;
    }
    set ordinalMonth(value){
        this.#ordinalMonth = value;
    }
    #day;
    get day() {
        return this.#day;
    }
    set day(value){
        this.#day = value;
    }
    /** @internal */
    static fromFields(structObj) {
        return new DateFields(structObj);
    }

    #internalConstructor(structObj) {
        if (typeof structObj !== "object") {
            throw new Error("DateFields's constructor takes an object of DateFields's fields.");
        }

        if ("era" in structObj) {
            this.#era = structObj.era;
        } else {
            this.#era = null;
        }

        if ("eraYear" in structObj) {
            this.#eraYear = structObj.eraYear;
        } else {
            this.#eraYear = null;
        }

        if ("extendedYear" in structObj) {
            this.#extendedYear = structObj.extendedYear;
        } else {
            this.#extendedYear = null;
        }

        if ("monthCode" in structObj) {
            this.#monthCode = structObj.monthCode;
        } else {
            this.#monthCode = null;
        }

        if ("ordinalMonth" in structObj) {
            this.#ordinalMonth = structObj.ordinalMonth;
        } else {
            this.#ordinalMonth = null;
        }

        if ("day" in structObj) {
            this.#day = structObj.day;
        } else {
            this.#day = null;
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)// If this struct contains any slices, their lifetime-edge-relevant information will be
    // set up here, and can be appended to any relevant lifetime arrays here. <lifetime>AppendArray accepts a list
    // of arrays for each lifetime to do so. It accepts multiple lists per lifetime in case the caller needs to tie a lifetime to multiple
    // output arrays. Null is equivalent to an empty list: this lifetime is not being borrowed from.
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        let buffer = DiplomatBuf.struct(wasm$1, 44, 4);

        this._writeToArrayBuffer(wasm$1.memory.buffer, buffer.ptr, functionCleanupArena, appendArrayMap);

        functionCleanupArena.alloc(buffer);

        return buffer.ptr;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof DateFields) {
            return obj;
        }

        return DateFields.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeOptionToArrayBuffer(arrayBuffer, offset + 0, this.#era, 8, 4, (arrayBuffer, offset, jsValue) => CleanupArena.maybeCreateWith(functionCleanupArena, ...appendArrayMap['aAppendArray']).alloc(DiplomatBuf.str8(wasm$1, jsValue)).writePtrLenToArrayBuffer(arrayBuffer, offset + 0));
        writeOptionToArrayBuffer(arrayBuffer, offset + 12, this.#eraYear, 4, 4, (arrayBuffer, offset, jsValue) => writeToArrayBuffer(arrayBuffer, offset + 0, jsValue, Int32Array));
        writeOptionToArrayBuffer(arrayBuffer, offset + 20, this.#extendedYear, 4, 4, (arrayBuffer, offset, jsValue) => writeToArrayBuffer(arrayBuffer, offset + 0, jsValue, Int32Array));
        writeOptionToArrayBuffer(arrayBuffer, offset + 28, this.#monthCode, 8, 4, (arrayBuffer, offset, jsValue) => CleanupArena.maybeCreateWith(functionCleanupArena, ...appendArrayMap['aAppendArray']).alloc(DiplomatBuf.str8(wasm$1, jsValue)).writePtrLenToArrayBuffer(arrayBuffer, offset + 0));
        writeOptionToArrayBuffer(arrayBuffer, offset + 40, this.#ordinalMonth, 1, 1, (arrayBuffer, offset, jsValue) => writeToArrayBuffer(arrayBuffer, offset + 0, jsValue, Uint8Array));
        writeOptionToArrayBuffer(arrayBuffer, offset + 42, this.#day, 1, 1, (arrayBuffer, offset, jsValue) => writeToArrayBuffer(arrayBuffer, offset + 0, jsValue, Uint8Array));
    }

    static _fromFFI(internalConstructor$1, ptr, aEdges) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("DateFields._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        const eraDeref = ptr;
        structObj.era = readOption(wasm$1, eraDeref, 8, (wasm, offset) => { const deref = offset; return new DiplomatSliceStr(wasm, deref,  "string8", aEdges).getValue() });
        const eraYearDeref = ptr + 12;
        structObj.eraYear = readOption(wasm$1, eraYearDeref, 4, (wasm, offset) => { const deref = (new Int32Array(wasm.memory.buffer, offset, 1))[0]; return deref });
        const extendedYearDeref = ptr + 20;
        structObj.extendedYear = readOption(wasm$1, extendedYearDeref, 4, (wasm, offset) => { const deref = (new Int32Array(wasm.memory.buffer, offset, 1))[0]; return deref });
        const monthCodeDeref = ptr + 28;
        structObj.monthCode = readOption(wasm$1, monthCodeDeref, 8, (wasm, offset) => { const deref = offset; return new DiplomatSliceStr(wasm, deref,  "string8", aEdges).getValue() });
        const ordinalMonthDeref = ptr + 40;
        structObj.ordinalMonth = readOption(wasm$1, ordinalMonthDeref, 1, (wasm, offset) => { const deref = (new Uint8Array(wasm.memory.buffer, offset, 1))[0]; return deref });
        const dayDeref = ptr + 42;
        structObj.day = readOption(wasm$1, dayDeref, 1, (wasm, offset) => { const deref = (new Uint8Array(wasm.memory.buffer, offset, 1))[0]; return deref });

        return new DateFields(structObj);
    }

    // Return all fields corresponding to lifetime `'a`
    // without handling lifetime dependencies (this is the job of the caller)
    // This is all fields that may be borrowed from if borrowing `'a`,
    // assuming that there are no `'other: a`. bounds. In case of such bounds,
    // the caller should take care to also call _fieldsForLifetimeOther
    get _fieldsForLifetimeA() {
        return [this.#era, this.#monthCode];
    };


    constructor(structObj) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `MissingFieldsStrategy`](https://docs.rs/icu/2.1.1/icu/calendar/options/enum.MissingFieldsStrategy.html) for more information.
 */
class DateMissingFieldsStrategy {
    #value = undefined;

    static #values = new Map([
        ["Reject", 0],
        ["Ecma", 1]
    ]);

    static getAllEntries() {
        return DateMissingFieldsStrategy.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DateMissingFieldsStrategy.#objectValues[arguments[1]];
        }

        if (value instanceof DateMissingFieldsStrategy) {
            return value;
        }

        let intVal = DateMissingFieldsStrategy.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DateMissingFieldsStrategy.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DateMissingFieldsStrategy and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DateMissingFieldsStrategy(value);
    }

    get value(){
        return [...DateMissingFieldsStrategy.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DateMissingFieldsStrategy(internalConstructor, internalConstructor, 0),
        new DateMissingFieldsStrategy(internalConstructor, internalConstructor, 1),
    ];

    static Reject = DateMissingFieldsStrategy.#objectValues[0];
    static Ecma = DateMissingFieldsStrategy.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `Overflow`](https://docs.rs/icu/2.1.1/icu/calendar/options/enum.Overflow.html) for more information.
 */
class DateOverflow {
    #value = undefined;

    static #values = new Map([
        ["Constrain", 0],
        ["Reject", 1]
    ]);

    static getAllEntries() {
        return DateOverflow.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DateOverflow.#objectValues[arguments[1]];
        }

        if (value instanceof DateOverflow) {
            return value;
        }

        let intVal = DateOverflow.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DateOverflow.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DateOverflow and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DateOverflow(value);
    }

    get value(){
        return [...DateOverflow.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DateOverflow(internalConstructor, internalConstructor, 0),
        new DateOverflow(internalConstructor, internalConstructor, 1),
    ];

    static Constrain = DateOverflow.#objectValues[0];
    static Reject = DateOverflow.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `DateFromFieldsOptions`](https://docs.rs/icu/2.1.1/icu/calendar/options/struct.DateFromFieldsOptions.html) for more information.
 */
class DateFromFieldsOptions {
    #overflow;
    get overflow() {
        return this.#overflow;
    }
    set overflow(value){
        this.#overflow = value;
    }
    #missingFieldsStrategy;
    get missingFieldsStrategy() {
        return this.#missingFieldsStrategy;
    }
    set missingFieldsStrategy(value){
        this.#missingFieldsStrategy = value;
    }
    /** @internal */
    static fromFields(structObj) {
        return new DateFromFieldsOptions(structObj);
    }

    #internalConstructor(structObj) {
        if (typeof structObj !== "object") {
            throw new Error("DateFromFieldsOptions's constructor takes an object of DateFromFieldsOptions's fields.");
        }

        if ("overflow" in structObj) {
            this.#overflow = structObj.overflow;
        } else {
            this.#overflow = null;
        }

        if ("missingFieldsStrategy" in structObj) {
            this.#missingFieldsStrategy = structObj.missingFieldsStrategy;
        } else {
            this.#missingFieldsStrategy = null;
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        let buffer = DiplomatBuf.struct(wasm$1, 16, 4);

        this._writeToArrayBuffer(wasm$1.memory.buffer, buffer.ptr, functionCleanupArena, appendArrayMap);

        functionCleanupArena.alloc(buffer);

        return buffer.ptr;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof DateFromFieldsOptions) {
            return obj;
        }

        return DateFromFieldsOptions.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeOptionToArrayBuffer(arrayBuffer, offset + 0, this.#overflow, 4, 4, (arrayBuffer, offset, jsValue) => writeToArrayBuffer(arrayBuffer, offset + 0, jsValue.ffiValue, Int32Array));
        writeOptionToArrayBuffer(arrayBuffer, offset + 8, this.#missingFieldsStrategy, 4, 4, (arrayBuffer, offset, jsValue) => writeToArrayBuffer(arrayBuffer, offset + 0, jsValue.ffiValue, Int32Array));
    }

    // This struct contains borrowed fields, so this takes in a list of
    // "edges" corresponding to where each lifetime's data may have been borrowed from
    // and passes it down to individual fields containing the borrow.
    // This method does not attempt to handle any dependencies between lifetimes, the caller
    // should handle this when constructing edge arrays.
    static _fromFFI(internalConstructor$1, ptr) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("DateFromFieldsOptions._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        const overflowDeref = ptr;
        structObj.overflow = readOption(wasm$1, overflowDeref, 4, (wasm, offset) => { const deref = enumDiscriminant(wasm, offset); return new DateOverflow(internalConstructor, deref) });
        const missingFieldsStrategyDeref = ptr + 8;
        structObj.missingFieldsStrategy = readOption(wasm$1, missingFieldsStrategyDeref, 4, (wasm, offset) => { const deref = enumDiscriminant(wasm, offset); return new DateMissingFieldsStrategy(internalConstructor, deref) });

        return new DateFromFieldsOptions(structObj);
    }


    constructor(structObj) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `IsoWeekOfYear`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.IsoWeekOfYear.html) for more information.
 */
class IsoWeekOfYear {
    #weekNumber;
    get weekNumber() {
        return this.#weekNumber;
    }
    set weekNumber(value){
        this.#weekNumber = value;
    }
    #isoYear;
    get isoYear() {
        return this.#isoYear;
    }
    set isoYear(value){
        this.#isoYear = value;
    }
    /** @internal */
    static fromFields(structObj) {
        return new IsoWeekOfYear(structObj);
    }

    #internalConstructor(structObj) {
        if (typeof structObj !== "object") {
            throw new Error("IsoWeekOfYear's constructor takes an object of IsoWeekOfYear's fields.");
        }

        if ("weekNumber" in structObj) {
            this.#weekNumber = structObj.weekNumber;
        } else {
            throw new Error("Missing required field weekNumber.");
        }

        if ("isoYear" in structObj) {
            this.#isoYear = structObj.isoYear;
        } else {
            throw new Error("Missing required field isoYear.");
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        let buffer = DiplomatBuf.struct(wasm$1, 8, 4);

        this._writeToArrayBuffer(wasm$1.memory.buffer, buffer.ptr, functionCleanupArena, appendArrayMap);

        functionCleanupArena.alloc(buffer);

        return buffer.ptr;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof IsoWeekOfYear) {
            return obj;
        }

        return IsoWeekOfYear.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeToArrayBuffer(arrayBuffer, offset + 0, this.#weekNumber, Uint8Array);
        writeToArrayBuffer(arrayBuffer, offset + 4, this.#isoYear, Int32Array);
    }

    // This struct contains borrowed fields, so this takes in a list of
    // "edges" corresponding to where each lifetime's data may have been borrowed from
    // and passes it down to individual fields containing the borrow.
    // This method does not attempt to handle any dependencies between lifetimes, the caller
    // should handle this when constructing edge arrays.
    static _fromFFI(internalConstructor$1, ptr) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("IsoWeekOfYear._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        const weekNumberDeref = (new Uint8Array(wasm$1.memory.buffer, ptr, 1))[0];
        structObj.weekNumber = weekNumberDeref;
        const isoYearDeref = (new Int32Array(wasm$1.memory.buffer, ptr + 4, 1))[0];
        structObj.isoYear = isoYearDeref;

        return new IsoWeekOfYear(structObj);
    }


    constructor(structObj) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `Fallback`](https://docs.rs/icu/2.1.1/icu/experimental/displaynames/enum.Fallback.html) for more information.
 */
class DisplayNamesFallback {
    #value = undefined;

    static #values = new Map([
        ["Code", 0],
        ["None", 1]
    ]);

    static getAllEntries() {
        return DisplayNamesFallback.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DisplayNamesFallback.#objectValues[arguments[1]];
        }

        if (value instanceof DisplayNamesFallback) {
            return value;
        }

        let intVal = DisplayNamesFallback.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DisplayNamesFallback.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DisplayNamesFallback and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DisplayNamesFallback(value);
    }

    get value(){
        return [...DisplayNamesFallback.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DisplayNamesFallback(internalConstructor, internalConstructor, 0),
        new DisplayNamesFallback(internalConstructor, internalConstructor, 1),
    ];

    static Code = DisplayNamesFallback.#objectValues[0];
    static None = DisplayNamesFallback.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `Style`](https://docs.rs/icu/2.1.1/icu/experimental/displaynames/enum.Style.html) for more information.
 */
class DisplayNamesStyle {
    #value = undefined;

    static #values = new Map([
        ["Narrow", 0],
        ["Short", 1],
        ["Long", 2],
        ["Menu", 3]
    ]);

    static getAllEntries() {
        return DisplayNamesStyle.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DisplayNamesStyle.#objectValues[arguments[1]];
        }

        if (value instanceof DisplayNamesStyle) {
            return value;
        }

        let intVal = DisplayNamesStyle.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DisplayNamesStyle.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DisplayNamesStyle and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DisplayNamesStyle(value);
    }

    get value(){
        return [...DisplayNamesStyle.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DisplayNamesStyle(internalConstructor, internalConstructor, 0),
        new DisplayNamesStyle(internalConstructor, internalConstructor, 1),
        new DisplayNamesStyle(internalConstructor, internalConstructor, 2),
        new DisplayNamesStyle(internalConstructor, internalConstructor, 3),
    ];

    static Narrow = DisplayNamesStyle.#objectValues[0];
    static Short = DisplayNamesStyle.#objectValues[1];
    static Long = DisplayNamesStyle.#objectValues[2];
    static Menu = DisplayNamesStyle.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * 🚧 This API is experimental and may experience breaking changes outside major releases.
 *
 * See the [Rust documentation for `LanguageDisplay`](https://docs.rs/icu/2.1.1/icu/experimental/displaynames/enum.LanguageDisplay.html) for more information.
 */
class LanguageDisplay {
    #value = undefined;

    static #values = new Map([
        ["Dialect", 0],
        ["Standard", 1]
    ]);

    static getAllEntries() {
        return LanguageDisplay.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LanguageDisplay.#objectValues[arguments[1]];
        }

        if (value instanceof LanguageDisplay) {
            return value;
        }

        let intVal = LanguageDisplay.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LanguageDisplay.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LanguageDisplay and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LanguageDisplay(value);
    }

    get value(){
        return [...LanguageDisplay.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LanguageDisplay(internalConstructor, internalConstructor, 0),
        new LanguageDisplay(internalConstructor, internalConstructor, 1),
    ];

    static Dialect = LanguageDisplay.#objectValues[0];
    static Standard = LanguageDisplay.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/locale/enum.ParseError.html)
 */
class LocaleParseError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["Language", 1],
        ["Subtag", 2],
        ["Extension", 3]
    ]);

    static getAllEntries() {
        return LocaleParseError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LocaleParseError.#objectValues[arguments[1]];
        }

        if (value instanceof LocaleParseError) {
            return value;
        }

        let intVal = LocaleParseError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LocaleParseError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LocaleParseError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LocaleParseError(value);
    }

    get value(){
        return [...LocaleParseError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LocaleParseError(internalConstructor, internalConstructor, 0),
        new LocaleParseError(internalConstructor, internalConstructor, 1),
        new LocaleParseError(internalConstructor, internalConstructor, 2),
        new LocaleParseError(internalConstructor, internalConstructor, 3),
    ];

    static Unknown = LocaleParseError.#objectValues[0];
    static Language = LocaleParseError.#objectValues[1];
    static Subtag = LocaleParseError.#objectValues[2];
    static Extension = LocaleParseError.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Locale_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * The various calendar types currently supported by {@link Calendar}
 *
 * See the [Rust documentation for `AnyCalendarKind`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendarKind.html) for more information.
 */
class CalendarKind {
    #value = undefined;

    static #values = new Map([
        ["Iso", 0],
        ["Gregorian", 1],
        ["Buddhist", 2],
        ["Japanese", 3],
        ["JapaneseExtended", 4],
        ["Ethiopian", 5],
        ["EthiopianAmeteAlem", 6],
        ["Indian", 7],
        ["Coptic", 8],
        ["Dangi", 9],
        ["Chinese", 10],
        ["Hebrew", 11],
        ["HijriTabularTypeIiFriday", 12],
        ["HijriSimulatedMecca", 18],
        ["HijriTabularTypeIiThursday", 14],
        ["HijriUmmAlQura", 15],
        ["Persian", 16],
        ["Roc", 17]
    ]);

    static getAllEntries() {
        return CalendarKind.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CalendarKind.#objectValues[arguments[1]];
        }

        if (value instanceof CalendarKind) {
            return value;
        }

        let intVal = CalendarKind.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CalendarKind.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CalendarKind and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CalendarKind(value);
    }

    get value(){
        for (let entry of CalendarKind.#values) {
            if (entry[1] == this.#value) {
                return entry[0];
            }
        }
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = {
        [0]: new CalendarKind(internalConstructor, internalConstructor, 0),
        [1]: new CalendarKind(internalConstructor, internalConstructor, 1),
        [2]: new CalendarKind(internalConstructor, internalConstructor, 2),
        [3]: new CalendarKind(internalConstructor, internalConstructor, 3),
        [4]: new CalendarKind(internalConstructor, internalConstructor, 4),
        [5]: new CalendarKind(internalConstructor, internalConstructor, 5),
        [6]: new CalendarKind(internalConstructor, internalConstructor, 6),
        [7]: new CalendarKind(internalConstructor, internalConstructor, 7),
        [8]: new CalendarKind(internalConstructor, internalConstructor, 8),
        [9]: new CalendarKind(internalConstructor, internalConstructor, 9),
        [10]: new CalendarKind(internalConstructor, internalConstructor, 10),
        [11]: new CalendarKind(internalConstructor, internalConstructor, 11),
        [12]: new CalendarKind(internalConstructor, internalConstructor, 12),
        [18]: new CalendarKind(internalConstructor, internalConstructor, 18),
        [14]: new CalendarKind(internalConstructor, internalConstructor, 14),
        [15]: new CalendarKind(internalConstructor, internalConstructor, 15),
        [16]: new CalendarKind(internalConstructor, internalConstructor, 16),
        [17]: new CalendarKind(internalConstructor, internalConstructor, 17),
    };

    static Iso = CalendarKind.#objectValues[0];
    static Gregorian = CalendarKind.#objectValues[1];
    static Buddhist = CalendarKind.#objectValues[2];
    static Japanese = CalendarKind.#objectValues[3];
    static JapaneseExtended = CalendarKind.#objectValues[4];
    static Ethiopian = CalendarKind.#objectValues[5];
    static EthiopianAmeteAlem = CalendarKind.#objectValues[6];
    static Indian = CalendarKind.#objectValues[7];
    static Coptic = CalendarKind.#objectValues[8];
    static Dangi = CalendarKind.#objectValues[9];
    static Chinese = CalendarKind.#objectValues[10];
    static Hebrew = CalendarKind.#objectValues[11];
    static HijriTabularTypeIiFriday = CalendarKind.#objectValues[12];
    static HijriSimulatedMecca = CalendarKind.#objectValues[18];
    static HijriTabularTypeIiThursday = CalendarKind.#objectValues[14];
    static HijriUmmAlQura = CalendarKind.#objectValues[15];
    static Persian = CalendarKind.#objectValues[16];
    static Roc = CalendarKind.#objectValues[17];


    /**
     * Creates a new {@link CalendarKind} for the specified locale, using compiled data.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendarKind.html#method.new) for more information.
     */
    static create(locale) {

        const result = wasm$1.icu4x_CalendarKind_create_mv1(locale.ffiValue);

        try {
            return new CalendarKind(internalConstructor, result);
        }

        finally {
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.InvalidOffsetError.html)
 */
class TimeZoneInvalidOffsetError {


}

// generated by diplomat-tool



/**
 * Priority mode for the ICU4X fallback algorithm.
 *
 * See the [Rust documentation for `LocaleFallbackPriority`](https://docs.rs/icu/2.1.1/icu/locale/fallback/enum.LocaleFallbackPriority.html) for more information.
 */
class LocaleFallbackPriority {
    #value = undefined;

    static #values = new Map([
        ["Language", 0],
        ["Region", 1]
    ]);

    static getAllEntries() {
        return LocaleFallbackPriority.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LocaleFallbackPriority.#objectValues[arguments[1]];
        }

        if (value instanceof LocaleFallbackPriority) {
            return value;
        }

        let intVal = LocaleFallbackPriority.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LocaleFallbackPriority.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LocaleFallbackPriority and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LocaleFallbackPriority(value);
    }

    get value(){
        return [...LocaleFallbackPriority.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LocaleFallbackPriority(internalConstructor, internalConstructor, 0),
        new LocaleFallbackPriority(internalConstructor, internalConstructor, 1),
    ];

    static Language = LocaleFallbackPriority.#objectValues[0];
    static Region = LocaleFallbackPriority.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `BidiPairedBracketType`](https://docs.rs/icu/2.1.1/icu/properties/props/enum.BidiPairedBracketType.html) for more information.
 */
class BidiPairedBracketType {
    #value = undefined;

    static #values = new Map([
        ["Open", 0],
        ["Close", 1],
        ["None", 2]
    ]);

    static getAllEntries() {
        return BidiPairedBracketType.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return BidiPairedBracketType.#objectValues[arguments[1]];
        }

        if (value instanceof BidiPairedBracketType) {
            return value;
        }

        let intVal = BidiPairedBracketType.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return BidiPairedBracketType.#objectValues[intVal];
        }

        throw TypeError(value + " is not a BidiPairedBracketType and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new BidiPairedBracketType(value);
    }

    get value(){
        return [...BidiPairedBracketType.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new BidiPairedBracketType(internalConstructor, internalConstructor, 0),
        new BidiPairedBracketType(internalConstructor, internalConstructor, 1),
        new BidiPairedBracketType(internalConstructor, internalConstructor, 2),
    ];

    static Open = BidiPairedBracketType.#objectValues[0];
    static Close = BidiPairedBracketType.#objectValues[1];
    static None = BidiPairedBracketType.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `GeneralCategory`](https://docs.rs/icu/2.1.1/icu/properties/props/enum.GeneralCategory.html) for more information.
 */
class GeneralCategory {
    #value = undefined;

    static #values = new Map([
        ["Unassigned", 0],
        ["UppercaseLetter", 1],
        ["LowercaseLetter", 2],
        ["TitlecaseLetter", 3],
        ["ModifierLetter", 4],
        ["OtherLetter", 5],
        ["NonspacingMark", 6],
        ["SpacingMark", 8],
        ["EnclosingMark", 7],
        ["DecimalNumber", 9],
        ["LetterNumber", 10],
        ["OtherNumber", 11],
        ["SpaceSeparator", 12],
        ["LineSeparator", 13],
        ["ParagraphSeparator", 14],
        ["Control", 15],
        ["Format", 16],
        ["PrivateUse", 17],
        ["Surrogate", 18],
        ["DashPunctuation", 19],
        ["OpenPunctuation", 20],
        ["ClosePunctuation", 21],
        ["ConnectorPunctuation", 22],
        ["InitialPunctuation", 28],
        ["FinalPunctuation", 29],
        ["OtherPunctuation", 23],
        ["MathSymbol", 24],
        ["CurrencySymbol", 25],
        ["ModifierSymbol", 26],
        ["OtherSymbol", 27]
    ]);

    static getAllEntries() {
        return GeneralCategory.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return GeneralCategory.#objectValues[arguments[1]];
        }

        if (value instanceof GeneralCategory) {
            return value;
        }

        let intVal = GeneralCategory.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return GeneralCategory.#objectValues[intVal];
        }

        throw TypeError(value + " is not a GeneralCategory and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new GeneralCategory(value);
    }

    get value(){
        for (let entry of GeneralCategory.#values) {
            if (entry[1] == this.#value) {
                return entry[0];
            }
        }
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = {
        [0]: new GeneralCategory(internalConstructor, internalConstructor, 0),
        [1]: new GeneralCategory(internalConstructor, internalConstructor, 1),
        [2]: new GeneralCategory(internalConstructor, internalConstructor, 2),
        [3]: new GeneralCategory(internalConstructor, internalConstructor, 3),
        [4]: new GeneralCategory(internalConstructor, internalConstructor, 4),
        [5]: new GeneralCategory(internalConstructor, internalConstructor, 5),
        [6]: new GeneralCategory(internalConstructor, internalConstructor, 6),
        [8]: new GeneralCategory(internalConstructor, internalConstructor, 8),
        [7]: new GeneralCategory(internalConstructor, internalConstructor, 7),
        [9]: new GeneralCategory(internalConstructor, internalConstructor, 9),
        [10]: new GeneralCategory(internalConstructor, internalConstructor, 10),
        [11]: new GeneralCategory(internalConstructor, internalConstructor, 11),
        [12]: new GeneralCategory(internalConstructor, internalConstructor, 12),
        [13]: new GeneralCategory(internalConstructor, internalConstructor, 13),
        [14]: new GeneralCategory(internalConstructor, internalConstructor, 14),
        [15]: new GeneralCategory(internalConstructor, internalConstructor, 15),
        [16]: new GeneralCategory(internalConstructor, internalConstructor, 16),
        [17]: new GeneralCategory(internalConstructor, internalConstructor, 17),
        [18]: new GeneralCategory(internalConstructor, internalConstructor, 18),
        [19]: new GeneralCategory(internalConstructor, internalConstructor, 19),
        [20]: new GeneralCategory(internalConstructor, internalConstructor, 20),
        [21]: new GeneralCategory(internalConstructor, internalConstructor, 21),
        [22]: new GeneralCategory(internalConstructor, internalConstructor, 22),
        [28]: new GeneralCategory(internalConstructor, internalConstructor, 28),
        [29]: new GeneralCategory(internalConstructor, internalConstructor, 29),
        [23]: new GeneralCategory(internalConstructor, internalConstructor, 23),
        [24]: new GeneralCategory(internalConstructor, internalConstructor, 24),
        [25]: new GeneralCategory(internalConstructor, internalConstructor, 25),
        [26]: new GeneralCategory(internalConstructor, internalConstructor, 26),
        [27]: new GeneralCategory(internalConstructor, internalConstructor, 27),
    };

    static Unassigned = GeneralCategory.#objectValues[0];
    static UppercaseLetter = GeneralCategory.#objectValues[1];
    static LowercaseLetter = GeneralCategory.#objectValues[2];
    static TitlecaseLetter = GeneralCategory.#objectValues[3];
    static ModifierLetter = GeneralCategory.#objectValues[4];
    static OtherLetter = GeneralCategory.#objectValues[5];
    static NonspacingMark = GeneralCategory.#objectValues[6];
    static SpacingMark = GeneralCategory.#objectValues[8];
    static EnclosingMark = GeneralCategory.#objectValues[7];
    static DecimalNumber = GeneralCategory.#objectValues[9];
    static LetterNumber = GeneralCategory.#objectValues[10];
    static OtherNumber = GeneralCategory.#objectValues[11];
    static SpaceSeparator = GeneralCategory.#objectValues[12];
    static LineSeparator = GeneralCategory.#objectValues[13];
    static ParagraphSeparator = GeneralCategory.#objectValues[14];
    static Control = GeneralCategory.#objectValues[15];
    static Format = GeneralCategory.#objectValues[16];
    static PrivateUse = GeneralCategory.#objectValues[17];
    static Surrogate = GeneralCategory.#objectValues[18];
    static DashPunctuation = GeneralCategory.#objectValues[19];
    static OpenPunctuation = GeneralCategory.#objectValues[20];
    static ClosePunctuation = GeneralCategory.#objectValues[21];
    static ConnectorPunctuation = GeneralCategory.#objectValues[22];
    static InitialPunctuation = GeneralCategory.#objectValues[28];
    static FinalPunctuation = GeneralCategory.#objectValues[29];
    static OtherPunctuation = GeneralCategory.#objectValues[23];
    static MathSymbol = GeneralCategory.#objectValues[24];
    static CurrencySymbol = GeneralCategory.#objectValues[25];
    static ModifierSymbol = GeneralCategory.#objectValues[26];
    static OtherSymbol = GeneralCategory.#objectValues[27];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_GeneralCategory_for_char_mv1(ch);

        try {
            return new GeneralCategory(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Convert to an integer using the ICU4C integer mappings for `General_Category`
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_GeneralCategory_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_GeneralCategory_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_GeneralCategory_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Produces a GeneralCategoryGroup mask that can represent a group of general categories
     *
     * See the [Rust documentation for `GeneralCategoryGroup`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html) for more information.
     */
    toGroup() {

        const result = wasm$1.icu4x_GeneralCategory_to_group_mv1(this.ffiValue);

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Convert from an integer using the ICU4C integer mappings for `General_Category`
     * Convert from an integer value from ICU4C or CodePointMapData
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_GeneralCategory_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new GeneralCategory(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * A mask that is capable of representing groups of `General_Category` values.
 *
 * See the [Rust documentation for `GeneralCategoryGroup`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html) for more information.
 */
class GeneralCategoryGroup {
    #mask;
    get mask() {
        return this.#mask;
    }
    set mask(value){
        this.#mask = value;
    }
    /** @internal */
    static fromFields(structObj) {
        return new GeneralCategoryGroup(structObj);
    }

    #internalConstructor(structObj) {
        if (typeof structObj !== "object") {
            throw new Error("GeneralCategoryGroup's constructor takes an object of GeneralCategoryGroup's fields.");
        }

        if ("mask" in structObj) {
            this.#mask = structObj.mask;
        } else {
            throw new Error("Missing required field mask.");
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        return this.#mask;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof GeneralCategoryGroup) {
            return obj;
        }

        return GeneralCategoryGroup.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeToArrayBuffer(arrayBuffer, offset + 0, this.#mask, Uint32Array);
    }

    // This struct contains borrowed fields, so this takes in a list of
    // "edges" corresponding to where each lifetime's data may have been borrowed from
    // and passes it down to individual fields containing the borrow.
    // This method does not attempt to handle any dependencies between lifetimes, the caller
    // should handle this when constructing edge arrays.
    static _fromFFI(internalConstructor$1, primitiveValue) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("GeneralCategoryGroup._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        structObj.mask = primitiveValue;

        return new GeneralCategoryGroup(structObj);
    }


    /**
     * See the [Rust documentation for `contains`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#method.contains) for more information.
     */
    contains(val) {
        let functionCleanupArena = new CleanupArena();


        const result = wasm$1.icu4x_GeneralCategoryGroup_contains_mv1(GeneralCategoryGroup._fromSuppliedValue(internalConstructor, this)._intoFFI(functionCleanupArena, {}, false), val.ffiValue);

        try {
            return result;
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `complement`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#method.complement) for more information.
     */
    complement() {
        let functionCleanupArena = new CleanupArena();


        const result = wasm$1.icu4x_GeneralCategoryGroup_complement_mv1(GeneralCategoryGroup._fromSuppliedValue(internalConstructor, this)._intoFFI(functionCleanupArena, {}, false));

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `all`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#method.all) for more information.
     */
    static all() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_all_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `empty`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#method.empty) for more information.
     */
    static empty() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_empty_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `union`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#method.union) for more information.
     */
    union(other) {
        let functionCleanupArena = new CleanupArena();


        const result = wasm$1.icu4x_GeneralCategoryGroup_union_mv1(GeneralCategoryGroup._fromSuppliedValue(internalConstructor, this)._intoFFI(functionCleanupArena, {}, false), GeneralCategoryGroup._fromSuppliedValue(internalConstructor, other)._intoFFI(functionCleanupArena, {}, false));

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `intersection`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#method.intersection) for more information.
     */
    intersection(other) {
        let functionCleanupArena = new CleanupArena();


        const result = wasm$1.icu4x_GeneralCategoryGroup_intersection_mv1(GeneralCategoryGroup._fromSuppliedValue(internalConstructor, this)._intoFFI(functionCleanupArena, {}, false), GeneralCategoryGroup._fromSuppliedValue(internalConstructor, other)._intoFFI(functionCleanupArena, {}, false));

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `CasedLetter`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.CasedLetter) for more information.
     */
    static casedLetter() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_cased_letter_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Letter`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Letter) for more information.
     */
    static letter() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_letter_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Mark`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Mark) for more information.
     */
    static mark() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_mark_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Number`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Number) for more information.
     */
    static number() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_number_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Other`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Other) for more information.
     */
    static separator() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_separator_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Letter`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Letter) for more information.
     */
    static other() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_other_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Punctuation`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Punctuation) for more information.
     */
    static punctuation() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_punctuation_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `Symbol`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GeneralCategoryGroup.html#associatedconstant.Symbol) for more information.
     */
    static symbol() {

        const result = wasm$1.icu4x_GeneralCategoryGroup_symbol_mv1();

        try {
            return GeneralCategoryGroup._fromFFI(internalConstructor, result);
        }

        finally {
        }
    }

    constructor(structObj) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `LineBreakStrictness`](https://docs.rs/icu/2.1.1/icu/segmenter/options/enum.LineBreakStrictness.html) for more information.
 */
class LineBreakStrictness {
    #value = undefined;

    static #values = new Map([
        ["Loose", 0],
        ["Normal", 1],
        ["Strict", 2],
        ["Anywhere", 3]
    ]);

    static getAllEntries() {
        return LineBreakStrictness.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LineBreakStrictness.#objectValues[arguments[1]];
        }

        if (value instanceof LineBreakStrictness) {
            return value;
        }

        let intVal = LineBreakStrictness.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LineBreakStrictness.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LineBreakStrictness and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LineBreakStrictness(value);
    }

    get value(){
        return [...LineBreakStrictness.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LineBreakStrictness(internalConstructor, internalConstructor, 0),
        new LineBreakStrictness(internalConstructor, internalConstructor, 1),
        new LineBreakStrictness(internalConstructor, internalConstructor, 2),
        new LineBreakStrictness(internalConstructor, internalConstructor, 3),
    ];

    static Loose = LineBreakStrictness.#objectValues[0];
    static Normal = LineBreakStrictness.#objectValues[1];
    static Strict = LineBreakStrictness.#objectValues[2];
    static Anywhere = LineBreakStrictness.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `LineBreakWordOption`](https://docs.rs/icu/2.1.1/icu/segmenter/options/enum.LineBreakWordOption.html) for more information.
 */
class LineBreakWordOption {
    #value = undefined;

    static #values = new Map([
        ["Normal", 0],
        ["BreakAll", 1],
        ["KeepAll", 2]
    ]);

    static getAllEntries() {
        return LineBreakWordOption.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LineBreakWordOption.#objectValues[arguments[1]];
        }

        if (value instanceof LineBreakWordOption) {
            return value;
        }

        let intVal = LineBreakWordOption.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LineBreakWordOption.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LineBreakWordOption and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LineBreakWordOption(value);
    }

    get value(){
        return [...LineBreakWordOption.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LineBreakWordOption(internalConstructor, internalConstructor, 0),
        new LineBreakWordOption(internalConstructor, internalConstructor, 1),
        new LineBreakWordOption(internalConstructor, internalConstructor, 2),
    ];

    static Normal = LineBreakWordOption.#objectValues[0];
    static BreakAll = LineBreakWordOption.#objectValues[1];
    static KeepAll = LineBreakWordOption.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `CollationCaseFirst`](https://docs.rs/icu/2.1.1/icu/collator/preferences/enum.CollationCaseFirst.html) for more information.
 */
class CollatorCaseFirst {
    #value = undefined;

    static #values = new Map([
        ["Off", 0],
        ["Lower", 1],
        ["Upper", 2]
    ]);

    static getAllEntries() {
        return CollatorCaseFirst.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CollatorCaseFirst.#objectValues[arguments[1]];
        }

        if (value instanceof CollatorCaseFirst) {
            return value;
        }

        let intVal = CollatorCaseFirst.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CollatorCaseFirst.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CollatorCaseFirst and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CollatorCaseFirst(value);
    }

    get value(){
        return [...CollatorCaseFirst.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CollatorCaseFirst(internalConstructor, internalConstructor, 0),
        new CollatorCaseFirst(internalConstructor, internalConstructor, 1),
        new CollatorCaseFirst(internalConstructor, internalConstructor, 2),
    ];

    static Off = CollatorCaseFirst.#objectValues[0];
    static Lower = CollatorCaseFirst.#objectValues[1];
    static Upper = CollatorCaseFirst.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `CollationNumericOrdering`](https://docs.rs/icu/2.1.1/icu/collator/preferences/enum.CollationNumericOrdering.html) for more information.
 */
class CollatorNumericOrdering {
    #value = undefined;

    static #values = new Map([
        ["Off", 0],
        ["On", 1]
    ]);

    static getAllEntries() {
        return CollatorNumericOrdering.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CollatorNumericOrdering.#objectValues[arguments[1]];
        }

        if (value instanceof CollatorNumericOrdering) {
            return value;
        }

        let intVal = CollatorNumericOrdering.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CollatorNumericOrdering.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CollatorNumericOrdering and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CollatorNumericOrdering(value);
    }

    get value(){
        return [...CollatorNumericOrdering.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CollatorNumericOrdering(internalConstructor, internalConstructor, 0),
        new CollatorNumericOrdering(internalConstructor, internalConstructor, 1),
    ];

    static Off = CollatorNumericOrdering.#objectValues[0];
    static On = CollatorNumericOrdering.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu_provider/2.1.1/icu_provider/struct.DataError.html), [2](https://docs.rs/icu_provider/2.1.1/icu_provider/enum.DataErrorKind.html)
 */
class DataError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["MarkerNotFound", 1],
        ["IdentifierNotFound", 2],
        ["InvalidRequest", 3],
        ["InconsistentData", 4],
        ["Downcast", 5],
        ["Deserialize", 6],
        ["Custom", 7],
        ["Io", 8]
    ]);

    static getAllEntries() {
        return DataError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DataError.#objectValues[arguments[1]];
        }

        if (value instanceof DataError) {
            return value;
        }

        let intVal = DataError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DataError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DataError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DataError(value);
    }

    get value(){
        return [...DataError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DataError(internalConstructor, internalConstructor, 0),
        new DataError(internalConstructor, internalConstructor, 1),
        new DataError(internalConstructor, internalConstructor, 2),
        new DataError(internalConstructor, internalConstructor, 3),
        new DataError(internalConstructor, internalConstructor, 4),
        new DataError(internalConstructor, internalConstructor, 5),
        new DataError(internalConstructor, internalConstructor, 6),
        new DataError(internalConstructor, internalConstructor, 7),
        new DataError(internalConstructor, internalConstructor, 8),
    ];

    static Unknown = DataError.#objectValues[0];
    static MarkerNotFound = DataError.#objectValues[1];
    static IdentifierNotFound = DataError.#objectValues[2];
    static InvalidRequest = DataError.#objectValues[3];
    static InconsistentData = DataError.#objectValues[4];
    static Downcast = DataError.#objectValues[5];
    static Deserialize = DataError.#objectValues[6];
    static Custom = DataError.#objectValues[7];
    static Io = DataError.#objectValues[8];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleFallbackIterator_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleFallbackerWithConfig_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleFallbacker_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DataProvider_destroy_mv1(ptr);
});

// generated by diplomat-tool

const Calendar_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Calendar_destroy_mv1(ptr);
});

/**
 * See the [Rust documentation for `AnyCalendar`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendar.html) for more information.
 */
class Calendar {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("Calendar is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            Calendar_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Creates a new {@link Calendar} for the specified kind, using compiled data.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendar.html#method.new) for more information.
     */
    #defaultConstructor(kind) {

        const result = wasm$1.icu4x_Calendar_create_mv1(kind.ffiValue);

        try {
            return new Calendar(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Creates a new {@link Calendar} for the specified kind, using a particular data source.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendar.html#method.new) for more information.
     */
    static createWithProvider(provider, kind) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Calendar_create_with_provider_mv1(diplomatReceive.buffer, provider.ffiValue, kind.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new DataError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('DataError.' + cause.value, { cause });
            }
            return new Calendar(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Returns the kind of this calendar
     *
     * See the [Rust documentation for `kind`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendar.html#method.kind) for more information.
     */
    get kind() {

        const result = wasm$1.icu4x_Calendar_kind_mv1(this.ffiValue);

        try {
            return new CalendarKind(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Creates a new {@link Calendar} for the specified kind, using compiled data.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/calendar/enum.AnyCalendar.html#method.new) for more information.
     */
    constructor(kind) {
        if (arguments[0] === exposeConstructor) {
            return this.#internalConstructor(...Array.prototype.slice.call(arguments, 1));
        } else if (arguments[0] === internalConstructor) {
            return this.#internalConstructor(...arguments);
        } else {
            return this.#defaultConstructor(...arguments);
        }
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/error/enum.DateFromFieldsError.html)
 */
class CalendarDateFromFieldsError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["OutOfRange", 1],
        ["UnknownEra", 2],
        ["MonthCodeInvalidSyntax", 3],
        ["MonthCodeNotInCalendar", 4],
        ["MonthCodeNotInYear", 5],
        ["InconsistentYear", 6],
        ["InconsistentMonth", 7],
        ["NotEnoughFields", 8]
    ]);

    static getAllEntries() {
        return CalendarDateFromFieldsError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CalendarDateFromFieldsError.#objectValues[arguments[1]];
        }

        if (value instanceof CalendarDateFromFieldsError) {
            return value;
        }

        let intVal = CalendarDateFromFieldsError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CalendarDateFromFieldsError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CalendarDateFromFieldsError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CalendarDateFromFieldsError(value);
    }

    get value(){
        return [...CalendarDateFromFieldsError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 0),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 1),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 2),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 3),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 4),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 5),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 6),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 7),
        new CalendarDateFromFieldsError(internalConstructor, internalConstructor, 8),
    ];

    static Unknown = CalendarDateFromFieldsError.#objectValues[0];
    static OutOfRange = CalendarDateFromFieldsError.#objectValues[1];
    static UnknownEra = CalendarDateFromFieldsError.#objectValues[2];
    static MonthCodeInvalidSyntax = CalendarDateFromFieldsError.#objectValues[3];
    static MonthCodeNotInCalendar = CalendarDateFromFieldsError.#objectValues[4];
    static MonthCodeNotInYear = CalendarDateFromFieldsError.#objectValues[5];
    static InconsistentYear = CalendarDateFromFieldsError.#objectValues[6];
    static InconsistentMonth = CalendarDateFromFieldsError.#objectValues[7];
    static NotEnoughFields = CalendarDateFromFieldsError.#objectValues[8];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/struct.RangeError.html), [2](https://docs.rs/icu/2.1.1/icu/calendar/enum.DateError.html)
 */
class CalendarError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["OutOfRange", 1],
        ["UnknownEra", 2],
        ["UnknownMonthCode", 3]
    ]);

    static getAllEntries() {
        return CalendarError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CalendarError.#objectValues[arguments[1]];
        }

        if (value instanceof CalendarError) {
            return value;
        }

        let intVal = CalendarError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CalendarError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CalendarError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CalendarError(value);
    }

    get value(){
        return [...CalendarError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new CalendarError(internalConstructor, internalConstructor, 0),
        new CalendarError(internalConstructor, internalConstructor, 1),
        new CalendarError(internalConstructor, internalConstructor, 2),
        new CalendarError(internalConstructor, internalConstructor, 3),
    ];

    static Unknown = CalendarError.#objectValues[0];
    static OutOfRange = CalendarError.#objectValues[1];
    static UnknownEra = CalendarError.#objectValues[2];
    static UnknownMonthCode = CalendarError.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/enum.ParseError.html), [2](https://docs.rs/icu/2.1.1/icu/time/enum.ParseError.html)
 */
class Rfc9557ParseError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["InvalidSyntax", 1],
        ["OutOfRange", 2],
        ["MissingFields", 3],
        ["UnknownCalendar", 4]
    ]);

    static getAllEntries() {
        return Rfc9557ParseError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return Rfc9557ParseError.#objectValues[arguments[1]];
        }

        if (value instanceof Rfc9557ParseError) {
            return value;
        }

        let intVal = Rfc9557ParseError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return Rfc9557ParseError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a Rfc9557ParseError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new Rfc9557ParseError(value);
    }

    get value(){
        return [...Rfc9557ParseError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new Rfc9557ParseError(internalConstructor, internalConstructor, 0),
        new Rfc9557ParseError(internalConstructor, internalConstructor, 1),
        new Rfc9557ParseError(internalConstructor, internalConstructor, 2),
        new Rfc9557ParseError(internalConstructor, internalConstructor, 3),
        new Rfc9557ParseError(internalConstructor, internalConstructor, 4),
    ];

    static Unknown = Rfc9557ParseError.#objectValues[0];
    static InvalidSyntax = Rfc9557ParseError.#objectValues[1];
    static OutOfRange = Rfc9557ParseError.#objectValues[2];
    static MissingFields = Rfc9557ParseError.#objectValues[3];
    static UnknownCalendar = Rfc9557ParseError.#objectValues[4];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Weekday`](https://docs.rs/icu/2.1.1/icu/calendar/types/enum.Weekday.html) for more information.
 */
class Weekday {
    #value = undefined;

    static #values = new Map([
        ["Monday", 1],
        ["Tuesday", 2],
        ["Wednesday", 3],
        ["Thursday", 4],
        ["Friday", 5],
        ["Saturday", 6],
        ["Sunday", 7]
    ]);

    static getAllEntries() {
        return Weekday.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return Weekday.#objectValues[arguments[1]];
        }

        if (value instanceof Weekday) {
            return value;
        }

        let intVal = Weekday.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return Weekday.#objectValues[intVal];
        }

        throw TypeError(value + " is not a Weekday and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new Weekday(value);
    }

    get value(){
        for (let entry of Weekday.#values) {
            if (entry[1] == this.#value) {
                return entry[0];
            }
        }
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = {
        [1]: new Weekday(internalConstructor, internalConstructor, 1),
        [2]: new Weekday(internalConstructor, internalConstructor, 2),
        [3]: new Weekday(internalConstructor, internalConstructor, 3),
        [4]: new Weekday(internalConstructor, internalConstructor, 4),
        [5]: new Weekday(internalConstructor, internalConstructor, 5),
        [6]: new Weekday(internalConstructor, internalConstructor, 6),
        [7]: new Weekday(internalConstructor, internalConstructor, 7),
    };

    static Monday = Weekday.#objectValues[1];
    static Tuesday = Weekday.#objectValues[2];
    static Wednesday = Weekday.#objectValues[3];
    static Thursday = Weekday.#objectValues[4];
    static Friday = Weekday.#objectValues[5];
    static Saturday = Weekday.#objectValues[6];
    static Sunday = Weekday.#objectValues[7];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

const IsoDate_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_IsoDate_destroy_mv1(ptr);
});

/**
 * An ICU4X Date object capable of containing a ISO-8601 date
 *
 * See the [Rust documentation for `Date`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html) for more information.
 */
class IsoDate {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("IsoDate is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            IsoDate_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Creates a new {@link IsoDate} from the specified date.
     *
     * See the [Rust documentation for `try_new_iso`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.try_new_iso) for more information.
     */
    #defaultConstructor(year, month, day) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_IsoDate_create_mv1(diplomatReceive.buffer, year, month, day);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new IsoDate(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link IsoDate} from the given Rata Die
     *
     * See the [Rust documentation for `from_rata_die`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.from_rata_die) for more information.
     */
    static fromRataDie(rd) {

        const result = wasm$1.icu4x_IsoDate_from_rata_die_mv1(rd);

        try {
            return new IsoDate(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Creates a new {@link IsoDate} from an IXDTF string.
     *
     * See the [Rust documentation for `try_from_str`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.try_from_str) for more information.
     */
    static fromString(v) {
        let functionCleanupArena = new CleanupArena();

        const vSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, v)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_IsoDate_from_string_mv1(diplomatReceive.buffer, vSlice.ptr);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new Rfc9557ParseError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('Rfc9557ParseError.' + cause.value, { cause });
            }
            return new IsoDate(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Convert this date to one in a different calendar
     *
     * See the [Rust documentation for `to_calendar`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.to_calendar) for more information.
     */
    toCalendar(calendar) {

        const result = wasm$1.icu4x_IsoDate_to_calendar_mv1(this.ffiValue, calendar.ffiValue);

        try {
            return new Date$1(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `to_any`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.to_any) for more information.
     */
    toAny() {

        const result = wasm$1.icu4x_IsoDate_to_any_mv1(this.ffiValue);

        try {
            return new Date$1(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Returns this date's Rata Die
     *
     * See the [Rust documentation for `to_rata_die`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.to_rata_die) for more information.
     */
    get rataDie() {

        const result = wasm$1.icu4x_IsoDate_to_rata_die_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the 1-indexed day in the year for this date
     *
     * See the [Rust documentation for `day_of_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.day_of_year) for more information.
     */
    get dayOfYear() {

        const result = wasm$1.icu4x_IsoDate_day_of_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the 1-indexed day in the month for this date
     *
     * See the [Rust documentation for `day_of_month`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.day_of_month) for more information.
     */
    get dayOfMonth() {

        const result = wasm$1.icu4x_IsoDate_day_of_month_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the day in the week for this day
     *
     * See the [Rust documentation for `day_of_week`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.day_of_week) for more information.
     */
    get dayOfWeek() {

        const result = wasm$1.icu4x_IsoDate_day_of_week_mv1(this.ffiValue);

        try {
            return new Weekday(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Returns the week number in this year, using week data
     *
     * See the [Rust documentation for `week_of_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.week_of_year) for more information.
     */
    weekOfYear() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 8, 4, false);


        wasm$1.icu4x_IsoDate_week_of_year_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            return IsoWeekOfYear._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Returns 1-indexed number of the month of this date in its year
     *
     * See the [Rust documentation for `ordinal`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.MonthInfo.html#structfield.ordinal) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.month)
     */
    get month() {

        const result = wasm$1.icu4x_IsoDate_month_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the year number in the current era for this date
     *
     * For calendars without an era, returns the extended year
     *
     * See the [Rust documentation for `year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.year) for more information.
     */
    get year() {

        const result = wasm$1.icu4x_IsoDate_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns if the year is a leap year for this date
     *
     * See the [Rust documentation for `is_in_leap_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.is_in_leap_year) for more information.
     */
    get isInLeapYear() {

        const result = wasm$1.icu4x_IsoDate_is_in_leap_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the number of months in the year represented by this date
     *
     * See the [Rust documentation for `months_in_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.months_in_year) for more information.
     */
    get monthsInYear() {

        const result = wasm$1.icu4x_IsoDate_months_in_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the number of days in the month represented by this date
     *
     * See the [Rust documentation for `days_in_month`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.days_in_month) for more information.
     */
    get daysInMonth() {

        const result = wasm$1.icu4x_IsoDate_days_in_month_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the number of days in the year represented by this date
     *
     * See the [Rust documentation for `days_in_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.days_in_year) for more information.
     */
    get daysInYear() {

        const result = wasm$1.icu4x_IsoDate_days_in_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Creates a new {@link IsoDate} from the specified date.
     *
     * See the [Rust documentation for `try_new_iso`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.try_new_iso) for more information.
     */
    constructor(year, month, day) {
        if (arguments[0] === exposeConstructor) {
            return this.#internalConstructor(...Array.prototype.slice.call(arguments, 1));
        } else if (arguments[0] === internalConstructor) {
            return this.#internalConstructor(...arguments);
        } else {
            return this.#defaultConstructor(...arguments);
        }
    }
}

// generated by diplomat-tool

const Date_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Date_destroy_mv1(ptr);
});

/**
 * An ICU4X Date object capable of containing a date for any calendar.
 *
 * See the [Rust documentation for `Date`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html) for more information.
 */
let Date$1 = class Date {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("Date is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            Date_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Creates a new {@link Date} representing the ISO date
     * given but in a given calendar
     *
     * See the [Rust documentation for `new_from_iso`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.new_from_iso) for more information.
     */
    static fromIsoInCalendar(isoYear, isoMonth, isoDay, calendar) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Date_from_iso_in_calendar_mv1(diplomatReceive.buffer, isoYear, isoMonth, isoDay, calendar.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new Date(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Date} from the given fields, which are interpreted in the given calendar system.
     *
     * 🚧 This API is experimental and may experience breaking changes outside major releases.
     *
     * See the [Rust documentation for `try_from_fields`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.try_from_fields) for more information.
     */
    static fromFieldsInCalendar(fields, options, calendar) {
        let functionCleanupArena = new CleanupArena();

        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Date_from_fields_in_calendar_mv1(diplomatReceive.buffer, DateFields._fromSuppliedValue(internalConstructor, fields)._intoFFI(functionCleanupArena, {}, false), DateFromFieldsOptions._fromSuppliedValue(internalConstructor, options)._intoFFI(functionCleanupArena, {}, false), calendar.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarDateFromFieldsError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarDateFromFieldsError.' + cause.value, { cause });
            }
            return new Date(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Date} from the given codes, which are interpreted in the given calendar system
     *
     * An empty era code will treat the year as an extended year
     *
     * See the [Rust documentation for `try_new_from_codes`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.try_new_from_codes) for more information.
     */
    static fromCodesInCalendar(eraCode, year, monthCode, day, calendar) {
        let functionCleanupArena = new CleanupArena();

        const eraCodeSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, eraCode)));
        const monthCodeSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, monthCode)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Date_from_codes_in_calendar_mv1(diplomatReceive.buffer, eraCodeSlice.ptr, year, monthCodeSlice.ptr, day, calendar.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new Date(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Date} from the given Rata Die
     *
     * See the [Rust documentation for `from_rata_die`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.from_rata_die) for more information.
     */
    static fromRataDie(rd, calendar) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Date_from_rata_die_mv1(diplomatReceive.buffer, rd, calendar.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new Date(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Date} from an IXDTF string.
     *
     * See the [Rust documentation for `try_from_str`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.try_from_str) for more information.
     */
    static fromString(v, calendar) {
        let functionCleanupArena = new CleanupArena();

        const vSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, v)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Date_from_string_mv1(diplomatReceive.buffer, vSlice.ptr, calendar.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new Rfc9557ParseError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('Rfc9557ParseError.' + cause.value, { cause });
            }
            return new Date(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Convert this date to one in a different calendar
     *
     * See the [Rust documentation for `to_calendar`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.to_calendar) for more information.
     */
    toCalendar(calendar) {

        const result = wasm$1.icu4x_Date_to_calendar_mv1(this.ffiValue, calendar.ffiValue);

        try {
            return new Date(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Converts this date to ISO
     *
     * See the [Rust documentation for `to_iso`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.to_iso) for more information.
     */
    toIso() {

        const result = wasm$1.icu4x_Date_to_iso_mv1(this.ffiValue);

        try {
            return new IsoDate(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Returns this date's Rata Die
     *
     * See the [Rust documentation for `to_rata_die`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.to_rata_die) for more information.
     */
    get rataDie() {

        const result = wasm$1.icu4x_Date_to_rata_die_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the 1-indexed day in the year for this date
     *
     * See the [Rust documentation for `day_of_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.day_of_year) for more information.
     */
    get dayOfYear() {

        const result = wasm$1.icu4x_Date_day_of_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the 1-indexed day in the month for this date
     *
     * See the [Rust documentation for `day_of_month`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.day_of_month) for more information.
     */
    get dayOfMonth() {

        const result = wasm$1.icu4x_Date_day_of_month_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the day in the week for this day
     *
     * See the [Rust documentation for `day_of_week`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.day_of_week) for more information.
     */
    get dayOfWeek() {

        const result = wasm$1.icu4x_Date_day_of_week_mv1(this.ffiValue);

        try {
            return new Weekday(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Returns 1-indexed number of the month of this date in its year
     *
     * Note that for lunar calendars this may not lead to the same month
     * having the same ordinal month across years; use month_code if you care
     * about month identity.
     *
     * See the [Rust documentation for `month`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.month) for more information.
     *
     * See the [Rust documentation for `ordinal`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.MonthInfo.html#structfield.ordinal) for more information.
     */
    get ordinalMonth() {

        const result = wasm$1.icu4x_Date_ordinal_month_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the month code for this date. Typically something
     * like "M01", "M02", but can be more complicated for lunar calendars.
     *
     * See the [Rust documentation for `standard_code`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.MonthInfo.html#structfield.standard_code) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.month)
     */
    get monthCode() {
        const write = new DiplomatWriteBuf(wasm$1);

    wasm$1.icu4x_Date_month_code_mv1(this.ffiValue, write.buffer);

        try {
            return write.readString8();
        }

        finally {
            write.free();
        }
    }

    /**
     * Returns the month number of this month.
     *
     * See the [Rust documentation for `month_number`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.MonthInfo.html#method.month_number) for more information.
     */
    get monthNumber() {

        const result = wasm$1.icu4x_Date_month_number_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns whether the month is a leap month.
     *
     * See the [Rust documentation for `is_leap`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.MonthInfo.html#method.is_leap) for more information.
     */
    get monthIsLeap() {

        const result = wasm$1.icu4x_Date_month_is_leap_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the year number in the current era for this date
     *
     * For calendars without an era, returns the related ISO year.
     *
     * See the [Rust documentation for `era_year_or_related_iso`](https://docs.rs/icu/2.1.1/icu/calendar/types/enum.YearInfo.html#method.era_year_or_related_iso) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.EraYear.html#structfield.year), [2](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.CyclicYear.html#structfield.related_iso), [3](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.year)
     */
    get eraYearOrRelatedIso() {

        const result = wasm$1.icu4x_Date_era_year_or_related_iso_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the extended year, which can be used for
     *
     * This year number can be used when you need a simple numeric representation
     * of the year, and can be meaningfully compared with extended years from other
     * eras or used in arithmetic.
     *
     * See the [Rust documentation for `extended_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.extended_year) for more information.
     */
    get extendedYear() {

        const result = wasm$1.icu4x_Date_extended_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the era for this date, or an empty string
     *
     * See the [Rust documentation for `era`](https://docs.rs/icu/2.1.1/icu/calendar/types/struct.EraYear.html#structfield.era) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.year)
     */
    get era() {
        const write = new DiplomatWriteBuf(wasm$1);

    wasm$1.icu4x_Date_era_mv1(this.ffiValue, write.buffer);

        try {
            return write.readString8();
        }

        finally {
            write.free();
        }
    }

    /**
     * Returns the number of months in the year represented by this date
     *
     * See the [Rust documentation for `months_in_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.months_in_year) for more information.
     */
    get monthsInYear() {

        const result = wasm$1.icu4x_Date_months_in_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the number of days in the month represented by this date
     *
     * See the [Rust documentation for `days_in_month`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.days_in_month) for more information.
     */
    get daysInMonth() {

        const result = wasm$1.icu4x_Date_days_in_month_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the number of days in the year represented by this date
     *
     * See the [Rust documentation for `days_in_year`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.days_in_year) for more information.
     */
    get daysInYear() {

        const result = wasm$1.icu4x_Date_days_in_year_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the {@link Calendar} object backing this date
     *
     * See the [Rust documentation for `calendar`](https://docs.rs/icu/2.1.1/icu/calendar/struct.Date.html#method.calendar) for more information.
     */
    get calendar() {

        const result = wasm$1.icu4x_Date_calendar_mv1(this.ffiValue);

        try {
            return new Calendar(internalConstructor, result, []);
        }

        finally {
        }
    }

    constructor(symbol, ptr, selfEdge) {
        return this.#internalConstructor(...arguments)
    }
};

// generated by diplomat-tool

const Time_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Time_destroy_mv1(ptr);
});

/**
 * An ICU4X Time object representing a time in terms of hour, minute, second, nanosecond
 *
 * See the [Rust documentation for `Time`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html) for more information.
 */
class Time {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("Time is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            Time_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Creates a new {@link Time} given field values
     *
     * See the [Rust documentation for `try_new`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#method.try_new) for more information.
     */
    #defaultConstructor(hour, minute, second, subsecond) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Time_create_mv1(diplomatReceive.buffer, hour, minute, second, subsecond);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new Time(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Time} from an IXDTF string.
     *
     * See the [Rust documentation for `try_from_str`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#method.try_from_str) for more information.
     */
    static fromString(v) {
        let functionCleanupArena = new CleanupArena();

        const vSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, v)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Time_from_string_mv1(diplomatReceive.buffer, vSlice.ptr);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new Rfc9557ParseError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('Rfc9557ParseError.' + cause.value, { cause });
            }
            return new Time(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Time} representing the start of the day (00:00:00.000).
     *
     * See the [Rust documentation for `start_of_day`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#method.start_of_day) for more information.
     */
    static startOfDay() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Time_start_of_day_mv1(diplomatReceive.buffer);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new Time(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link Time} representing noon (12:00:00.000).
     *
     * See the [Rust documentation for `noon`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#method.noon) for more information.
     */
    static noon() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Time_noon_mv1(diplomatReceive.buffer);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new CalendarError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('CalendarError.' + cause.value, { cause });
            }
            return new Time(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Returns the hour in this time
     *
     * See the [Rust documentation for `hour`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#structfield.hour) for more information.
     */
    get hour() {

        const result = wasm$1.icu4x_Time_hour_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the minute in this time
     *
     * See the [Rust documentation for `minute`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#structfield.minute) for more information.
     */
    get minute() {

        const result = wasm$1.icu4x_Time_minute_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the second in this time
     *
     * See the [Rust documentation for `second`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#structfield.second) for more information.
     */
    get second() {

        const result = wasm$1.icu4x_Time_second_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the subsecond in this time as nanoseconds
     *
     * See the [Rust documentation for `subsecond`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#structfield.subsecond) for more information.
     */
    get subsecond() {

        const result = wasm$1.icu4x_Time_subsecond_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Creates a new {@link Time} given field values
     *
     * See the [Rust documentation for `try_new`](https://docs.rs/icu/2.1.1/icu/time/struct.Time.html#method.try_new) for more information.
     */
    constructor(hour, minute, second, subsecond) {
        if (arguments[0] === exposeConstructor) {
            return this.#internalConstructor(...Array.prototype.slice.call(arguments, 1));
        } else if (arguments[0] === internalConstructor) {
            return this.#internalConstructor(...arguments);
        } else {
            return this.#defaultConstructor(...arguments);
        }
    }
}

// generated by diplomat-tool



/**
 * An ICU4X DateTime object capable of containing a ISO-8601 date and time.
 *
 * See the [Rust documentation for `DateTime`](https://docs.rs/icu/2.1.1/icu/time/struct.DateTime.html) for more information.
 */
class IsoDateTime {
    #date;
    get date() {
        return this.#date;
    }
    #time;
    get time() {
        return this.#time;
    }
    #internalConstructor(structObj, internalConstructor$1) {
        if (typeof structObj !== "object") {
            throw new Error("IsoDateTime's constructor takes an object of IsoDateTime's fields.");
        }

        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("IsoDateTime is an out struct and can only be created internally.");
        }
        if ("date" in structObj) {
            this.#date = structObj.date;
        } else {
            throw new Error("Missing required field date.");
        }

        if ("time" in structObj) {
            this.#time = structObj.time;
        } else {
            throw new Error("Missing required field time.");
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        let buffer = DiplomatBuf.struct(wasm$1, 8, 4);

        this._writeToArrayBuffer(wasm$1.memory.buffer, buffer.ptr, functionCleanupArena, appendArrayMap);

        functionCleanupArena.alloc(buffer);

        return buffer.ptr;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof IsoDateTime) {
            return obj;
        }

        return IsoDateTime.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeToArrayBuffer(arrayBuffer, offset + 0, this.#date.ffiValue, Uint32Array);
        writeToArrayBuffer(arrayBuffer, offset + 4, this.#time.ffiValue, Uint32Array);
    }

    // This struct contains borrowed fields, so this takes in a list of
    // "edges" corresponding to where each lifetime's data may have been borrowed from
    // and passes it down to individual fields containing the borrow.
    // This method does not attempt to handle any dependencies between lifetimes, the caller
    // should handle this when constructing edge arrays.
    static _fromFFI(internalConstructor$1, ptr) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("IsoDateTime._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        const dateDeref = ptrRead(wasm$1, ptr);
        structObj.date = new IsoDate(internalConstructor, dateDeref, []);
        const timeDeref = ptrRead(wasm$1, ptr + 4);
        structObj.time = new Time(internalConstructor, timeDeref, []);

        return new IsoDateTime(structObj, internalConstructor$1);
    }


    /**
     * Creates a new {@link IsoDateTime} from an IXDTF string.
     *
     * See the [Rust documentation for `try_from_str`](https://docs.rs/icu/2.1.1/icu/time/struct.DateTime.html#method.try_from_str) for more information.
     */
    static fromString(v) {
        let functionCleanupArena = new CleanupArena();

        const vSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, v)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_IsoDateTime_from_string_mv1(diplomatReceive.buffer, vSlice.ptr);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new Rfc9557ParseError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('Rfc9557ParseError.' + cause.value, { cause });
            }
            return IsoDateTime._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    constructor(structObj, internalConstructor) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `TimeZoneVariant`](https://docs.rs/icu/2.1.1/icu/time/zone/enum.TimeZoneVariant.html) for more information.
 *
 * @deprecated type not needed anymore
 */
class TimeZoneVariant {
    #value = undefined;

    static #values = new Map([
        ["Standard", 0],
        ["Daylight", 1]
    ]);

    static getAllEntries() {
        return TimeZoneVariant.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return TimeZoneVariant.#objectValues[arguments[1]];
        }

        if (value instanceof TimeZoneVariant) {
            return value;
        }

        let intVal = TimeZoneVariant.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return TimeZoneVariant.#objectValues[intVal];
        }

        throw TypeError(value + " is not a TimeZoneVariant and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new TimeZoneVariant(value);
    }

    get value(){
        return [...TimeZoneVariant.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new TimeZoneVariant(internalConstructor, internalConstructor, 0),
        new TimeZoneVariant(internalConstructor, internalConstructor, 1),
    ];

    static Standard = TimeZoneVariant.#objectValues[0];
    static Daylight = TimeZoneVariant.#objectValues[1];


    /**
     * See the [Rust documentation for `from_rearguard_isdst`](https://docs.rs/icu/2.1.1/icu/time/zone/enum.TimeZoneVariant.html#method.from_rearguard_isdst) for more information.
     *
     * See the [Rust documentation for `with_variant`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.with_variant) for more information.
     *
     * @deprecated type not needed anymore
     */
    static fromRearguardIsdst(isdst) {

        const result = wasm$1.icu4x_TimeZoneVariant_from_rearguard_isdst_mv1(isdst);

        try {
            return new TimeZoneVariant(internalConstructor, result);
        }

        finally {
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

const UtcOffset_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_UtcOffset_destroy_mv1(ptr);
});

/**
 * See the [Rust documentation for `UtcOffset`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html) for more information.
 */
class UtcOffset {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("UtcOffset is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            UtcOffset_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Creates an offset from seconds.
     *
     * Errors if the offset seconds are out of range.
     *
     * See the [Rust documentation for `try_from_seconds`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.try_from_seconds) for more information.
     */
    static fromSeconds(seconds) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_UtcOffset_from_seconds_mv1(diplomatReceive.buffer, seconds);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new TimeZoneInvalidOffsetError();
                throw new globalThis.Error('TimeZoneInvalidOffsetError', { cause });
            }
            return new UtcOffset(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates an offset from a string.
     *
     * See the [Rust documentation for `try_from_str`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.try_from_str) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    static fromString(offset) {
        let functionCleanupArena = new CleanupArena();

        const offsetSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, offset)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_UtcOffset_from_string_mv1(diplomatReceive.buffer, offsetSlice.ptr);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new TimeZoneInvalidOffsetError();
                throw new globalThis.Error('TimeZoneInvalidOffsetError', { cause });
            }
            return new UtcOffset(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Returns the value as offset seconds.
     *
     * See the [Rust documentation for `offset`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.offset) for more information.
     *
     * See the [Rust documentation for `to_seconds`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.to_seconds) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    get seconds() {

        const result = wasm$1.icu4x_UtcOffset_seconds_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns whether the offset is positive.
     *
     * See the [Rust documentation for `is_non_negative`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.is_non_negative) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    get isNonNegative() {

        const result = wasm$1.icu4x_UtcOffset_is_non_negative_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns whether the offset is zero.
     *
     * See the [Rust documentation for `is_zero`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.is_zero) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    get isZero() {

        const result = wasm$1.icu4x_UtcOffset_is_zero_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the hours part of the offset.
     *
     * See the [Rust documentation for `hours_part`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.hours_part) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    get hoursPart() {

        const result = wasm$1.icu4x_UtcOffset_hours_part_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the minutes part of the offset.
     *
     * See the [Rust documentation for `minutes_part`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.minutes_part) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    get minutesPart() {

        const result = wasm$1.icu4x_UtcOffset_minutes_part_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Returns the seconds part of the offset.
     *
     * See the [Rust documentation for `seconds_part`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html#method.seconds_part) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.UtcOffset.html)
     */
    get secondsPart() {

        const result = wasm$1.icu4x_UtcOffset_seconds_part_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    constructor(symbol, ptr, selfEdge) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `VariantOffsets`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsets.html) for more information.
 */
class VariantOffsets {
    #standard;
    get standard() {
        return this.#standard;
    }
    #daylight;
    get daylight() {
        return this.#daylight;
    }
    #internalConstructor(structObj, internalConstructor$1) {
        if (typeof structObj !== "object") {
            throw new Error("VariantOffsets's constructor takes an object of VariantOffsets's fields.");
        }

        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("VariantOffsets is an out struct and can only be created internally.");
        }
        if ("standard" in structObj) {
            this.#standard = structObj.standard;
        } else {
            throw new Error("Missing required field standard.");
        }

        if ("daylight" in structObj) {
            this.#daylight = structObj.daylight;
        } else {
            throw new Error("Missing required field daylight.");
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        let buffer = DiplomatBuf.struct(wasm$1, 8, 4);

        this._writeToArrayBuffer(wasm$1.memory.buffer, buffer.ptr, functionCleanupArena, appendArrayMap);

        functionCleanupArena.alloc(buffer);

        return buffer.ptr;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof VariantOffsets) {
            return obj;
        }

        return VariantOffsets.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeToArrayBuffer(arrayBuffer, offset + 0, this.#standard.ffiValue, Uint32Array);
        writeToArrayBuffer(arrayBuffer, offset + 4, this.#daylight.ffiValue ?? 0, Uint32Array);
    }

    // This struct contains borrowed fields, so this takes in a list of
    // "edges" corresponding to where each lifetime's data may have been borrowed from
    // and passes it down to individual fields containing the borrow.
    // This method does not attempt to handle any dependencies between lifetimes, the caller
    // should handle this when constructing edge arrays.
    static _fromFFI(internalConstructor$1, ptr) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("VariantOffsets._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        const standardDeref = ptrRead(wasm$1, ptr);
        structObj.standard = new UtcOffset(internalConstructor, standardDeref, []);
        const daylightDeref = ptrRead(wasm$1, ptr + 4);
        structObj.daylight = daylightDeref === 0 ? null : new UtcOffset(internalConstructor, daylightDeref, []);

        return new VariantOffsets(structObj, internalConstructor$1);
    }


    constructor(structObj, internalConstructor) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

const VariantOffsetsCalculator_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_VariantOffsetsCalculator_destroy_mv1(ptr);
});

/**
 * See the [Rust documentation for `VariantOffsetsCalculator`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsetsCalculator.html) for more information.
 *
 * @deprecated this API is a bad approximation of a time zone database
 */
class VariantOffsetsCalculator {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("VariantOffsetsCalculator is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            VariantOffsetsCalculator_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Construct a new {@link VariantOffsetsCalculator} instance using compiled data.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsetsCalculator.html#method.new) for more information.
     */
    #defaultConstructor() {

        const result = wasm$1.icu4x_VariantOffsetsCalculator_create_mv1();

        try {
            return new VariantOffsetsCalculator(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Construct a new {@link VariantOffsetsCalculator} instance using a particular data source.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsetsCalculator.html#method.new) for more information.
     */
    static createWithProvider(provider) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_VariantOffsetsCalculator_create_with_provider_mv1(diplomatReceive.buffer, provider.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new DataError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('DataError.' + cause.value, { cause });
            }
            return new VariantOffsetsCalculator(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * See the [Rust documentation for `compute_offsets_from_time_zone_and_name_timestamp`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsetsCalculatorBorrowed.html#method.compute_offsets_from_time_zone_and_name_timestamp) for more information.
     */
    computeOffsetsFromTimeZoneAndDateTime(timeZone, utcDate, utcTime) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_VariantOffsetsCalculator_compute_offsets_from_time_zone_and_date_time_mv1(diplomatReceive.buffer, this.ffiValue, timeZone.ffiValue, utcDate.ffiValue, utcTime.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return VariantOffsets._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * See the [Rust documentation for `compute_offsets_from_time_zone_and_name_timestamp`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsetsCalculatorBorrowed.html#method.compute_offsets_from_time_zone_and_name_timestamp) for more information.
     */
    computeOffsetsFromTimeZoneAndTimestamp(timeZone, timestamp) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_VariantOffsetsCalculator_compute_offsets_from_time_zone_and_timestamp_mv1(diplomatReceive.buffer, this.ffiValue, timeZone.ffiValue, timestamp);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return VariantOffsets._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Construct a new {@link VariantOffsetsCalculator} instance using compiled data.
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/time/zone/struct.VariantOffsetsCalculator.html#method.new) for more information.
     */
    constructor() {
        if (arguments[0] === exposeConstructor) {
            return this.#internalConstructor(...Array.prototype.slice.call(arguments, 1));
        } else if (arguments[0] === internalConstructor) {
            return this.#internalConstructor(...arguments);
        } else {
            return this.#defaultConstructor(...arguments);
        }
    }
}

// generated by diplomat-tool

const TimeZoneInfo_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeZoneInfo_destroy_mv1(ptr);
});

/**
 * See the [Rust documentation for `TimeZoneInfo`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html) for more information.
 */
class TimeZoneInfo {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("TimeZoneInfo is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            TimeZoneInfo_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Creates a time zone for UTC (Coordinated Universal Time).
     *
     * See the [Rust documentation for `utc`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.utc) for more information.
     */
    static utc() {

        const result = wasm$1.icu4x_TimeZoneInfo_utc_mv1();

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Creates a time zone info from parts.
     *
     * `variant` is ignored.
     */
    #defaultConstructor(id, offset, variant) {
        let functionCleanupArena = new CleanupArena();


        const result = wasm$1.icu4x_TimeZoneInfo_from_parts_mv1(id.ffiValue, offset.ffiValue ?? 0, optionToBufferForCalling(wasm$1, variant, 4, 4, functionCleanupArena, (arrayBuffer, offset, jsValue) => [writeToArrayBuffer(arrayBuffer, offset + 0, jsValue.ffiValue, Int32Array)]));

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `id`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.id) for more information.
     */
    get id() {

        const result = wasm$1.icu4x_TimeZoneInfo_id_mv1(this.ffiValue);

        try {
            return new TimeZone(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Sets the datetime at which to interpret the time zone
     * for display name lookup.
     *
     * Notes:
     *
     * - If not set, the formatting datetime is used if possible.
     * - If the offset is not set, the datetime is interpreted as UTC.
     * - The constraints are the same as with `ZoneNameTimestamp` in Rust.
     * - Set to year 1000 or 9999 for a reference far in the past or future.
     *
     * See the [Rust documentation for `at_date_time_iso`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.at_date_time_iso) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.ZoneNameTimestamp.html)
     */
    atDateTimeIso(date, time) {

        const result = wasm$1.icu4x_TimeZoneInfo_at_date_time_iso_mv1(this.ffiValue, date.ffiValue, time.ffiValue);

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Sets the timestamp, in milliseconds since Unix epoch, at which to interpret the time zone
     * for display name lookup.
     *
     * Notes:
     *
     * - If not set, the formatting datetime is used if possible.
     * - The constraints are the same as with `ZoneNameTimestamp` in Rust.
     *
     * See the [Rust documentation for `with_zone_name_timestamp`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.with_zone_name_timestamp) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/struct.ZoneNameTimestamp.html#method.from_zoned_date_time_iso), [2](https://docs.rs/icu/2.1.1/icu/time/zone/struct.ZoneNameTimestamp.html)
     */
    atTimestamp(timestamp) {

        const result = wasm$1.icu4x_TimeZoneInfo_at_timestamp_mv1(this.ffiValue, timestamp);

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Returns the DateTime for the UTC zone name reference time
     *
     * See the [Rust documentation for `zone_name_timestamp`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.zone_name_timestamp) for more information.
     */
    get zoneNameDateTime() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_TimeZoneInfo_zone_name_date_time_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return IsoDateTime._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * See the [Rust documentation for `with_variant`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.with_variant) for more information.
     *
     * @deprecated returns unmodified copy
     */
    withVariant(timeVariant) {

        const result = wasm$1.icu4x_TimeZoneInfo_with_variant_mv1(this.ffiValue, timeVariant.ffiValue);

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `offset`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.offset) for more information.
     */
    get offset() {

        const result = wasm$1.icu4x_TimeZoneInfo_offset_mv1(this.ffiValue);

        try {
            return result === 0 ? null : new UtcOffset(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `infer_variant`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.infer_variant) for more information.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/zone/enum.TimeZoneVariant.html)
     *
     * @deprecated does nothing
     */
    inferVariant(offsetCalculator) {

        const result = wasm$1.icu4x_TimeZoneInfo_infer_variant_mv1(this.ffiValue, offsetCalculator.ffiValue);

        try {
            return result === 1;
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `variant`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.variant) for more information.
     *
     * @deprecated always returns null
     */
    variant() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_TimeZoneInfo_variant_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new TimeZoneVariant(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Creates a time zone info from parts.
     *
     * `variant` is ignored.
     */
    constructor(id, offset, variant) {
        if (arguments[0] === exposeConstructor) {
            return this.#internalConstructor(...Array.prototype.slice.call(arguments, 1));
        } else if (arguments[0] === internalConstructor) {
            return this.#internalConstructor(...arguments);
        } else {
            return this.#defaultConstructor(...arguments);
        }
    }
}

// generated by diplomat-tool

const TimeZone_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeZone_destroy_mv1(ptr);
});

/**
 * See the [Rust documentation for `TimeZone`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZone.html) for more information.
 */
class TimeZone {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("TimeZone is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            TimeZone_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * The unknown time zone.
     *
     * See the [Rust documentation for `unknown`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZoneInfo.html#method.unknown) for more information.
     */
    static unknown() {

        const result = wasm$1.icu4x_TimeZone_unknown_mv1();

        try {
            return new TimeZone(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Whether the time zone is the unknown zone.
     *
     * See the [Rust documentation for `is_unknown`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZone.html#method.is_unknown) for more information.
     */
    isUnknown() {

        const result = wasm$1.icu4x_TimeZone_is_unknown_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Creates a time zone from a BCP-47 string.
     *
     * Returns the unknown time zone if the string is not a valid BCP-47 subtag.
     *
     * Additional information: [1](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZone.html)
     */
    static createFromBcp47(id) {
        let functionCleanupArena = new CleanupArena();

        const idSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, id)));

        const result = wasm$1.icu4x_TimeZone_create_from_bcp47_mv1(idSlice.ptr);

        try {
            return new TimeZone(internalConstructor, result, []);
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `with_offset`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZone.html#method.with_offset) for more information.
     */
    withOffset(offset) {

        const result = wasm$1.icu4x_TimeZone_with_offset_mv1(this.ffiValue, offset.ffiValue);

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * See the [Rust documentation for `without_offset`](https://docs.rs/icu/2.1.1/icu/time/struct.TimeZone.html#method.without_offset) for more information.
     */
    withoutOffset() {

        const result = wasm$1.icu4x_TimeZone_without_offset_mv1(this.ffiValue);

        try {
            return new TimeZoneInfo(internalConstructor, result, []);
        }

        finally {
        }
    }

    constructor(symbol, ptr, selfEdge) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

const TimeZoneIterator_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeZoneIterator_destroy_mv1(ptr);
});

/**
 * See the [Rust documentation for `TimeZoneIter`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.TimeZoneIter.html) for more information.
 */
class TimeZoneIterator {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];
    #aEdge = [];

    #internalConstructor(symbol, ptr, selfEdge, aEdge) {
        if (symbol !== internalConstructor) {
            console.error("TimeZoneIterator is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#aEdge = aEdge;
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            TimeZoneIterator_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * See the [Rust documentation for `next`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.TimeZoneIter.html#method.next) for more information.
     */
    #iteratorNext() {

        const result = wasm$1.icu4x_TimeZoneIterator_next_mv1(this.ffiValue);

        try {
            return result === 0 ? null : new TimeZone(internalConstructor, result, []);
        }

        finally {
        }
    }

    next(){
        const out = this.#iteratorNext();

        return {
            value: out,
            done: out === null,
        };
    }

    constructor(symbol, ptr, selfEdge, aEdge) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

const IanaParser_box_destroy_registry = new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_IanaParser_destroy_mv1(ptr);
});

/**
 * A mapper between IANA time zone identifiers and BCP-47 time zone identifiers.
 *
 * This mapper supports two-way mapping, but it is optimized for the case of IANA to BCP-47.
 * It also supports normalizing and canonicalizing the IANA strings.
 *
 * See the [Rust documentation for `IanaParser`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.IanaParser.html) for more information.
 */
class IanaParser {
    // Internal ptr reference:
    #ptr = null;

    // Lifetimes are only to keep dependencies alive.
    // Since JS won't garbage collect until there are no incoming edges.
    #selfEdge = [];

    #internalConstructor(symbol, ptr, selfEdge) {
        if (symbol !== internalConstructor) {
            console.error("IanaParser is an Opaque type. You cannot call its constructor.");
            return;
        }
        this.#ptr = ptr;
        this.#selfEdge = selfEdge;

        // Are we being borrowed? If not, we can register.
        if (this.#selfEdge.length === 0) {
            IanaParser_box_destroy_registry.register(this, this.#ptr);
        }

        return this;
    }
    /** @internal */
    get ffiValue() {
        return this.#ptr;
    }


    /**
     * Create a new {@link IanaParser} using compiled data
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.IanaParser.html#method.new) for more information.
     */
    #defaultConstructor() {

        const result = wasm$1.icu4x_IanaParser_create_mv1();

        try {
            return new IanaParser(internalConstructor, result, []);
        }

        finally {
        }
    }

    /**
     * Create a new {@link IanaParser} using a particular data source
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.IanaParser.html#method.new) for more information.
     */
    static createWithProvider(provider) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_IanaParser_create_with_provider_mv1(diplomatReceive.buffer, provider.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new DataError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('DataError.' + cause.value, { cause });
            }
            return new IanaParser(internalConstructor, ptrRead(wasm$1, diplomatReceive.buffer), []);
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * See the [Rust documentation for `parse`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.IanaParserBorrowed.html#method.parse) for more information.
     */
    parse(value) {
        let functionCleanupArena = new CleanupArena();

        const valueSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, value)));

        const result = wasm$1.icu4x_IanaParser_parse_mv1(this.ffiValue, valueSlice.ptr);

        try {
            return new TimeZone(internalConstructor, result, []);
        }

        finally {
            functionCleanupArena.free();

        }
    }

    /**
     * See the [Rust documentation for `iter`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.IanaParserBorrowed.html#method.iter) for more information.
     */
    iter() {
        // This lifetime edge depends on lifetimes 'a
        let aEdges = [this];


        const result = wasm$1.icu4x_IanaParser_iter_mv1(this.ffiValue);

        try {
            return new TimeZoneIterator(internalConstructor, result, [], aEdges);
        }

        finally {
        }
    }

    /**
     * Create a new {@link IanaParser} using compiled data
     *
     * See the [Rust documentation for `new`](https://docs.rs/icu/2.1.1/icu/time/zone/iana/struct.IanaParser.html#method.new) for more information.
     */
    constructor() {
        if (arguments[0] === exposeConstructor) {
            return this.#internalConstructor(...Array.prototype.slice.call(arguments, 1));
        } else if (arguments[0] === internalConstructor) {
            return this.#internalConstructor(...arguments);
        } else {
            return this.#defaultConstructor(...arguments);
        }
    }
}

// generated by diplomat-tool



/**
 * An ICU4X ZonedDateTime object capable of containing a ISO-8601 date, time, and zone.
 *
 * See the [Rust documentation for `ZonedDateTime`](https://docs.rs/icu/2.1.1/icu/time/struct.ZonedDateTime.html) for more information.
 */
class ZonedIsoDateTime {
    #date;
    get date() {
        return this.#date;
    }
    #time;
    get time() {
        return this.#time;
    }
    #zone;
    get zone() {
        return this.#zone;
    }
    #internalConstructor(structObj, internalConstructor$1) {
        if (typeof structObj !== "object") {
            throw new Error("ZonedIsoDateTime's constructor takes an object of ZonedIsoDateTime's fields.");
        }

        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("ZonedIsoDateTime is an out struct and can only be created internally.");
        }
        if ("date" in structObj) {
            this.#date = structObj.date;
        } else {
            throw new Error("Missing required field date.");
        }

        if ("time" in structObj) {
            this.#time = structObj.time;
        } else {
            throw new Error("Missing required field time.");
        }

        if ("zone" in structObj) {
            this.#zone = structObj.zone;
        } else {
            throw new Error("Missing required field zone.");
        }

        return this;
    }

    // Return this struct in FFI function friendly format.
    // Returns an array that can be expanded with spread syntax (...)
    _intoFFI(
        functionCleanupArena,
        appendArrayMap
    ) {
        let buffer = DiplomatBuf.struct(wasm$1, 12, 4);

        this._writeToArrayBuffer(wasm$1.memory.buffer, buffer.ptr, functionCleanupArena, appendArrayMap);

        functionCleanupArena.alloc(buffer);

        return buffer.ptr;
    }

    static _fromSuppliedValue(internalConstructor$1, obj) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("_fromSuppliedValue cannot be called externally.");
        }

        if (obj instanceof ZonedIsoDateTime) {
            return obj;
        }

        return ZonedIsoDateTime.fromFields(obj);
    }

    _writeToArrayBuffer(
        arrayBuffer,
        offset,
        functionCleanupArena,
        appendArrayMap
    ) {
        writeToArrayBuffer(arrayBuffer, offset + 0, this.#date.ffiValue, Uint32Array);
        writeToArrayBuffer(arrayBuffer, offset + 4, this.#time.ffiValue, Uint32Array);
        writeToArrayBuffer(arrayBuffer, offset + 8, this.#zone.ffiValue, Uint32Array);
    }

    // This struct contains borrowed fields, so this takes in a list of
    // "edges" corresponding to where each lifetime's data may have been borrowed from
    // and passes it down to individual fields containing the borrow.
    // This method does not attempt to handle any dependencies between lifetimes, the caller
    // should handle this when constructing edge arrays.
    static _fromFFI(internalConstructor$1, ptr) {
        if (internalConstructor$1 !== internalConstructor) {
            throw new Error("ZonedIsoDateTime._fromFFI is not meant to be called externally. Please use the default constructor.");
        }
        let structObj = {};
        const dateDeref = ptrRead(wasm$1, ptr);
        structObj.date = new IsoDate(internalConstructor, dateDeref, []);
        const timeDeref = ptrRead(wasm$1, ptr + 4);
        structObj.time = new Time(internalConstructor, timeDeref, []);
        const zoneDeref = ptrRead(wasm$1, ptr + 8);
        structObj.zone = new TimeZoneInfo(internalConstructor, zoneDeref, []);

        return new ZonedIsoDateTime(structObj, internalConstructor$1);
    }


    /**
     * Creates a new {@link ZonedIsoDateTime} from an IXDTF string.
     *
     * See the [Rust documentation for `try_strict_from_str`](https://docs.rs/icu/2.1.1/icu/time/struct.ZonedDateTime.html#method.try_strict_from_str) for more information.
     */
    static strictFromString(v, ianaParser) {
        let functionCleanupArena = new CleanupArena();

        const vSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, v)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 13, 4, true);


        wasm$1.icu4x_ZonedIsoDateTime_strict_from_string_mv1(diplomatReceive.buffer, vSlice.ptr, ianaParser.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new Rfc9557ParseError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('Rfc9557ParseError.' + cause.value, { cause });
            }
            return ZonedIsoDateTime._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link ZonedIsoDateTime} from an IXDTF string.
     *
     * See the [Rust documentation for `try_full_from_str`](https://docs.rs/icu/2.1.1/icu/time/struct.ZonedDateTime.html#method.try_full_from_str) for more information.
     *
     * @deprecated use strict_from_string
     */
    static fullFromString(v, ianaParser, offsetCalculator) {
        let functionCleanupArena = new CleanupArena();

        const vSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, v)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 13, 4, true);


        wasm$1.icu4x_ZonedIsoDateTime_full_from_string_mv1(diplomatReceive.buffer, vSlice.ptr, ianaParser.ffiValue, offsetCalculator.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                const cause = new Rfc9557ParseError(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
                throw new globalThis.Error('Rfc9557ParseError.' + cause.value, { cause });
            }
            return ZonedIsoDateTime._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    /**
     * Creates a new {@link ZonedIsoDateTime} from milliseconds since epoch (timestamp) and a UTC offset.
     *
     * Note: {@link ZonedIsoDateTime}s created with this constructor can only be formatted using localized offset zone styles.
     *
     * See the [Rust documentation for `from_epoch_milliseconds_and_utc_offset`](https://docs.rs/icu/2.1.1/icu/time/struct.ZonedDateTime.html#method.from_epoch_milliseconds_and_utc_offset) for more information.
     */
    static fromEpochMillisecondsAndUtcOffset(epochMilliseconds, utcOffset) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 12, 4, false);


        wasm$1.icu4x_ZonedIsoDateTime_from_epoch_milliseconds_and_utc_offset_mv1(diplomatReceive.buffer, epochMilliseconds, utcOffset.ffiValue);

        try {
            return ZonedIsoDateTime._fromFFI(internalConstructor, diplomatReceive.buffer);
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(structObj, internalConstructor) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Direction`](https://docs.rs/unicode_bidi/0.3.11/unicode_bidi/enum.Direction.html) for more information.
 */
class BidiDirection {
    #value = undefined;

    static #values = new Map([
        ["Ltr", 0],
        ["Rtl", 1],
        ["Mixed", 2]
    ]);

    static getAllEntries() {
        return BidiDirection.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return BidiDirection.#objectValues[arguments[1]];
        }

        if (value instanceof BidiDirection) {
            return value;
        }

        let intVal = BidiDirection.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return BidiDirection.#objectValues[intVal];
        }

        throw TypeError(value + " is not a BidiDirection and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new BidiDirection(value);
    }

    get value(){
        return [...BidiDirection.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new BidiDirection(internalConstructor, internalConstructor, 0),
        new BidiDirection(internalConstructor, internalConstructor, 1),
        new BidiDirection(internalConstructor, internalConstructor, 2),
    ];

    static Ltr = BidiDirection.#objectValues[0];
    static Rtl = BidiDirection.#objectValues[1];
    static Mixed = BidiDirection.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_BidiParagraph_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_BidiInfo_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ReorderedIndexMap_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Bidi_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CodePointRangeIterator_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CodePointSetData_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CodePointSetBuilder_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CaseMapCloser_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CaseMapper_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TitlecaseMapper_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Collator_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Alignment`](https://docs.rs/icu/2.1.1/icu/datetime/options/enum.Alignment.html) for more information.
 */
class DateTimeAlignment {
    #value = undefined;

    static #values = new Map([
        ["Auto", 0],
        ["Column", 1]
    ]);

    static getAllEntries() {
        return DateTimeAlignment.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DateTimeAlignment.#objectValues[arguments[1]];
        }

        if (value instanceof DateTimeAlignment) {
            return value;
        }

        let intVal = DateTimeAlignment.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DateTimeAlignment.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DateTimeAlignment and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DateTimeAlignment(value);
    }

    get value(){
        return [...DateTimeAlignment.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DateTimeAlignment(internalConstructor, internalConstructor, 0),
        new DateTimeAlignment(internalConstructor, internalConstructor, 1),
    ];

    static Auto = DateTimeAlignment.#objectValues[0];
    static Column = DateTimeAlignment.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/datetime/enum.DateTimeFormatterLoadError.html), [2](https://docs.rs/icu/2.1.1/icu/datetime/pattern/enum.PatternLoadError.html), [3](https://docs.rs/icu_provider/2.1.1/icu_provider/struct.DataError.html), [4](https://docs.rs/icu_provider/2.1.1/icu_provider/enum.DataErrorKind.html)
 */
class DateTimeFormatterLoadError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["InvalidDateFields", 2049],
        ["UnsupportedLength", 2051],
        ["ConflictingField", 2057],
        ["FormatterTooSpecific", 2058],
        ["DataMarkerNotFound", 1],
        ["DataIdentifierNotFound", 2],
        ["DataInvalidRequest", 3],
        ["DataInconsistentData", 4],
        ["DataDowncast", 5],
        ["DataDeserialize", 6],
        ["DataCustom", 7],
        ["DataIo", 8]
    ]);

    static getAllEntries() {
        return DateTimeFormatterLoadError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DateTimeFormatterLoadError.#objectValues[arguments[1]];
        }

        if (value instanceof DateTimeFormatterLoadError) {
            return value;
        }

        let intVal = DateTimeFormatterLoadError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DateTimeFormatterLoadError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DateTimeFormatterLoadError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DateTimeFormatterLoadError(value);
    }

    get value(){
        for (let entry of DateTimeFormatterLoadError.#values) {
            if (entry[1] == this.#value) {
                return entry[0];
            }
        }
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = {
        [0]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 0),
        [2049]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 2049),
        [2051]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 2051),
        [2057]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 2057),
        [2058]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 2058),
        [1]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 1),
        [2]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 2),
        [3]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 3),
        [4]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 4),
        [5]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 5),
        [6]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 6),
        [7]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 7),
        [8]: new DateTimeFormatterLoadError(internalConstructor, internalConstructor, 8),
    };

    static Unknown = DateTimeFormatterLoadError.#objectValues[0];
    static InvalidDateFields = DateTimeFormatterLoadError.#objectValues[2049];
    static UnsupportedLength = DateTimeFormatterLoadError.#objectValues[2051];
    static ConflictingField = DateTimeFormatterLoadError.#objectValues[2057];
    static FormatterTooSpecific = DateTimeFormatterLoadError.#objectValues[2058];
    static DataMarkerNotFound = DateTimeFormatterLoadError.#objectValues[1];
    static DataIdentifierNotFound = DateTimeFormatterLoadError.#objectValues[2];
    static DataInvalidRequest = DateTimeFormatterLoadError.#objectValues[3];
    static DataInconsistentData = DateTimeFormatterLoadError.#objectValues[4];
    static DataDowncast = DateTimeFormatterLoadError.#objectValues[5];
    static DataDeserialize = DateTimeFormatterLoadError.#objectValues[6];
    static DataCustom = DateTimeFormatterLoadError.#objectValues[7];
    static DataIo = DateTimeFormatterLoadError.#objectValues[8];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Length`](https://docs.rs/icu/2.1.1/icu/datetime/options/enum.Length.html) for more information.
 */
class DateTimeLength {
    #value = undefined;

    static #values = new Map([
        ["Long", 0],
        ["Medium", 1],
        ["Short", 2]
    ]);

    static getAllEntries() {
        return DateTimeLength.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DateTimeLength.#objectValues[arguments[1]];
        }

        if (value instanceof DateTimeLength) {
            return value;
        }

        let intVal = DateTimeLength.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DateTimeLength.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DateTimeLength and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DateTimeLength(value);
    }

    get value(){
        return [...DateTimeLength.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DateTimeLength(internalConstructor, internalConstructor, 0),
        new DateTimeLength(internalConstructor, internalConstructor, 1),
        new DateTimeLength(internalConstructor, internalConstructor, 2),
    ];

    static Long = DateTimeLength.#objectValues[0];
    static Medium = DateTimeLength.#objectValues[1];
    static Short = DateTimeLength.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `YearStyle`](https://docs.rs/icu/2.1.1/icu/datetime/options/enum.YearStyle.html) for more information.
 */
class YearStyle {
    #value = undefined;

    static #values = new Map([
        ["Auto", 0],
        ["Full", 1],
        ["WithEra", 2]
    ]);

    static getAllEntries() {
        return YearStyle.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return YearStyle.#objectValues[arguments[1]];
        }

        if (value instanceof YearStyle) {
            return value;
        }

        let intVal = YearStyle.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return YearStyle.#objectValues[intVal];
        }

        throw TypeError(value + " is not a YearStyle and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new YearStyle(value);
    }

    get value(){
        return [...YearStyle.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new YearStyle(internalConstructor, internalConstructor, 0),
        new YearStyle(internalConstructor, internalConstructor, 1),
        new YearStyle(internalConstructor, internalConstructor, 2),
    ];

    static Auto = YearStyle.#objectValues[0];
    static Full = YearStyle.#objectValues[1];
    static WithEra = YearStyle.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DateFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DateFormatterGregorian_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `TimePrecision`](https://docs.rs/icu/2.1.1/icu/datetime/options/enum.TimePrecision.html) for more information.
 *
 * See the [Rust documentation for `SubsecondDigits`](https://docs.rs/icu/2.1.1/icu/datetime/options/enum.SubsecondDigits.html) for more information.
 */
class TimePrecision {
    #value = undefined;

    static #values = new Map([
        ["Hour", 0],
        ["Minute", 1],
        ["MinuteOptional", 2],
        ["Second", 3],
        ["Subsecond1", 4],
        ["Subsecond2", 5],
        ["Subsecond3", 6],
        ["Subsecond4", 7],
        ["Subsecond5", 8],
        ["Subsecond6", 9],
        ["Subsecond7", 10],
        ["Subsecond8", 11],
        ["Subsecond9", 12]
    ]);

    static getAllEntries() {
        return TimePrecision.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return TimePrecision.#objectValues[arguments[1]];
        }

        if (value instanceof TimePrecision) {
            return value;
        }

        let intVal = TimePrecision.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return TimePrecision.#objectValues[intVal];
        }

        throw TypeError(value + " is not a TimePrecision and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new TimePrecision(value);
    }

    get value(){
        return [...TimePrecision.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new TimePrecision(internalConstructor, internalConstructor, 0),
        new TimePrecision(internalConstructor, internalConstructor, 1),
        new TimePrecision(internalConstructor, internalConstructor, 2),
        new TimePrecision(internalConstructor, internalConstructor, 3),
        new TimePrecision(internalConstructor, internalConstructor, 4),
        new TimePrecision(internalConstructor, internalConstructor, 5),
        new TimePrecision(internalConstructor, internalConstructor, 6),
        new TimePrecision(internalConstructor, internalConstructor, 7),
        new TimePrecision(internalConstructor, internalConstructor, 8),
        new TimePrecision(internalConstructor, internalConstructor, 9),
        new TimePrecision(internalConstructor, internalConstructor, 10),
        new TimePrecision(internalConstructor, internalConstructor, 11),
        new TimePrecision(internalConstructor, internalConstructor, 12),
    ];

    static Hour = TimePrecision.#objectValues[0];
    static Minute = TimePrecision.#objectValues[1];
    static MinuteOptional = TimePrecision.#objectValues[2];
    static Second = TimePrecision.#objectValues[3];
    static Subsecond1 = TimePrecision.#objectValues[4];
    static Subsecond2 = TimePrecision.#objectValues[5];
    static Subsecond3 = TimePrecision.#objectValues[6];
    static Subsecond4 = TimePrecision.#objectValues[7];
    static Subsecond5 = TimePrecision.#objectValues[8];
    static Subsecond6 = TimePrecision.#objectValues[9];
    static Subsecond7 = TimePrecision.#objectValues[10];
    static Subsecond8 = TimePrecision.#objectValues[11];
    static Subsecond9 = TimePrecision.#objectValues[12];


    /**
     * See the [Rust documentation for `try_from_int`](https://docs.rs/icu/2.1.1/icu/datetime/options/enum.SubsecondDigits.html#method.try_from_int) for more information.
     */
    static fromSubsecondDigits(digits) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_TimePrecision_from_subsecond_digits_mv1(diplomatReceive.buffer, digits);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new TimePrecision(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DateTimeFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DateTimeFormatterGregorian_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * Additional information: [1](https://docs.rs/fixed_decimal/0.7.0/fixed_decimal/enum.ParseError.html)
 */
class DecimalParseError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["Limit", 1],
        ["Syntax", 2]
    ]);

    static getAllEntries() {
        return DecimalParseError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DecimalParseError.#objectValues[arguments[1]];
        }

        if (value instanceof DecimalParseError) {
            return value;
        }

        let intVal = DecimalParseError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DecimalParseError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DecimalParseError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DecimalParseError(value);
    }

    get value(){
        return [...DecimalParseError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DecimalParseError(internalConstructor, internalConstructor, 0),
        new DecimalParseError(internalConstructor, internalConstructor, 1),
        new DecimalParseError(internalConstructor, internalConstructor, 2),
    ];

    static Unknown = DecimalParseError.#objectValues[0];
    static Limit = DecimalParseError.#objectValues[1];
    static Syntax = DecimalParseError.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Increment used in a rounding operation.
 *
 * See the [Rust documentation for `RoundingIncrement`](https://docs.rs/fixed_decimal/0.7.0/fixed_decimal/enum.RoundingIncrement.html) for more information.
 */
class DecimalRoundingIncrement {
    #value = undefined;

    static #values = new Map([
        ["MultiplesOf1", 0],
        ["MultiplesOf2", 1],
        ["MultiplesOf5", 2],
        ["MultiplesOf25", 3]
    ]);

    static getAllEntries() {
        return DecimalRoundingIncrement.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DecimalRoundingIncrement.#objectValues[arguments[1]];
        }

        if (value instanceof DecimalRoundingIncrement) {
            return value;
        }

        let intVal = DecimalRoundingIncrement.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DecimalRoundingIncrement.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DecimalRoundingIncrement and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DecimalRoundingIncrement(value);
    }

    get value(){
        return [...DecimalRoundingIncrement.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DecimalRoundingIncrement(internalConstructor, internalConstructor, 0),
        new DecimalRoundingIncrement(internalConstructor, internalConstructor, 1),
        new DecimalRoundingIncrement(internalConstructor, internalConstructor, 2),
        new DecimalRoundingIncrement(internalConstructor, internalConstructor, 3),
    ];

    static MultiplesOf1 = DecimalRoundingIncrement.#objectValues[0];
    static MultiplesOf2 = DecimalRoundingIncrement.#objectValues[1];
    static MultiplesOf5 = DecimalRoundingIncrement.#objectValues[2];
    static MultiplesOf25 = DecimalRoundingIncrement.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * The sign of a Decimal, as shown in formatting.
 *
 * See the [Rust documentation for `Sign`](https://docs.rs/fixed_decimal/0.7.0/fixed_decimal/enum.Sign.html) for more information.
 */
class DecimalSign {
    #value = undefined;

    static #values = new Map([
        ["None", 0],
        ["Negative", 1],
        ["Positive", 2]
    ]);

    static getAllEntries() {
        return DecimalSign.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DecimalSign.#objectValues[arguments[1]];
        }

        if (value instanceof DecimalSign) {
            return value;
        }

        let intVal = DecimalSign.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DecimalSign.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DecimalSign and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DecimalSign(value);
    }

    get value(){
        return [...DecimalSign.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DecimalSign(internalConstructor, internalConstructor, 0),
        new DecimalSign(internalConstructor, internalConstructor, 1),
        new DecimalSign(internalConstructor, internalConstructor, 2),
    ];

    static None = DecimalSign.#objectValues[0];
    static Negative = DecimalSign.#objectValues[1];
    static Positive = DecimalSign.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * ECMA-402 compatible sign display preference.
 *
 * See the [Rust documentation for `SignDisplay`](https://docs.rs/fixed_decimal/0.7.0/fixed_decimal/enum.SignDisplay.html) for more information.
 */
class DecimalSignDisplay {
    #value = undefined;

    static #values = new Map([
        ["Auto", 0],
        ["Never", 1],
        ["Always", 2],
        ["ExceptZero", 3],
        ["Negative", 4]
    ]);

    static getAllEntries() {
        return DecimalSignDisplay.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DecimalSignDisplay.#objectValues[arguments[1]];
        }

        if (value instanceof DecimalSignDisplay) {
            return value;
        }

        let intVal = DecimalSignDisplay.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DecimalSignDisplay.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DecimalSignDisplay and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DecimalSignDisplay(value);
    }

    get value(){
        return [...DecimalSignDisplay.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DecimalSignDisplay(internalConstructor, internalConstructor, 0),
        new DecimalSignDisplay(internalConstructor, internalConstructor, 1),
        new DecimalSignDisplay(internalConstructor, internalConstructor, 2),
        new DecimalSignDisplay(internalConstructor, internalConstructor, 3),
        new DecimalSignDisplay(internalConstructor, internalConstructor, 4),
    ];

    static Auto = DecimalSignDisplay.#objectValues[0];
    static Never = DecimalSignDisplay.#objectValues[1];
    static Always = DecimalSignDisplay.#objectValues[2];
    static ExceptZero = DecimalSignDisplay.#objectValues[3];
    static Negative = DecimalSignDisplay.#objectValues[4];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * Mode used in a rounding operation for signed numbers.
 *
 * See the [Rust documentation for `SignedRoundingMode`](https://docs.rs/fixed_decimal/0.7.0/fixed_decimal/enum.SignedRoundingMode.html) for more information.
 */
class DecimalSignedRoundingMode {
    #value = undefined;

    static #values = new Map([
        ["Expand", 0],
        ["Trunc", 1],
        ["HalfExpand", 2],
        ["HalfTrunc", 3],
        ["HalfEven", 4],
        ["Ceil", 5],
        ["Floor", 6],
        ["HalfCeil", 7],
        ["HalfFloor", 8]
    ]);

    static getAllEntries() {
        return DecimalSignedRoundingMode.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DecimalSignedRoundingMode.#objectValues[arguments[1]];
        }

        if (value instanceof DecimalSignedRoundingMode) {
            return value;
        }

        let intVal = DecimalSignedRoundingMode.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DecimalSignedRoundingMode.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DecimalSignedRoundingMode and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DecimalSignedRoundingMode(value);
    }

    get value(){
        return [...DecimalSignedRoundingMode.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 0),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 1),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 2),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 3),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 4),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 5),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 6),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 7),
        new DecimalSignedRoundingMode(internalConstructor, internalConstructor, 8),
    ];

    static Expand = DecimalSignedRoundingMode.#objectValues[0];
    static Trunc = DecimalSignedRoundingMode.#objectValues[1];
    static HalfExpand = DecimalSignedRoundingMode.#objectValues[2];
    static HalfTrunc = DecimalSignedRoundingMode.#objectValues[3];
    static HalfEven = DecimalSignedRoundingMode.#objectValues[4];
    static Ceil = DecimalSignedRoundingMode.#objectValues[5];
    static Floor = DecimalSignedRoundingMode.#objectValues[6];
    static HalfCeil = DecimalSignedRoundingMode.#objectValues[7];
    static HalfFloor = DecimalSignedRoundingMode.#objectValues[8];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Decimal_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `GroupingStrategy`](https://docs.rs/icu/2.1.1/icu/decimal/options/enum.GroupingStrategy.html) for more information.
 */
class DecimalGroupingStrategy {
    #value = undefined;

    static #values = new Map([
        ["Auto", 0],
        ["Never", 1],
        ["Always", 2],
        ["Min2", 3]
    ]);

    static getAllEntries() {
        return DecimalGroupingStrategy.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DecimalGroupingStrategy.#objectValues[arguments[1]];
        }

        if (value instanceof DecimalGroupingStrategy) {
            return value;
        }

        let intVal = DecimalGroupingStrategy.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DecimalGroupingStrategy.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DecimalGroupingStrategy and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DecimalGroupingStrategy(value);
    }

    get value(){
        return [...DecimalGroupingStrategy.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DecimalGroupingStrategy(internalConstructor, internalConstructor, 0),
        new DecimalGroupingStrategy(internalConstructor, internalConstructor, 1),
        new DecimalGroupingStrategy(internalConstructor, internalConstructor, 2),
        new DecimalGroupingStrategy(internalConstructor, internalConstructor, 3),
    ];

    static Auto = DecimalGroupingStrategy.#objectValues[0];
    static Never = DecimalGroupingStrategy.#objectValues[1];
    static Always = DecimalGroupingStrategy.#objectValues[2];
    static Min2 = DecimalGroupingStrategy.#objectValues[3];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DecimalFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleDisplayNamesFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_RegionDisplayNames_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ExemplarCharacters_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeZoneAndCanonicalAndNormalizedIterator_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeZoneAndCanonicalIterator_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_IanaParserExtended_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `ListLength`](https://docs.rs/icu/2.1.1/icu/list/options/enum.ListLength.html) for more information.
 */
class ListLength {
    #value = undefined;

    static #values = new Map([
        ["Wide", 0],
        ["Short", 1],
        ["Narrow", 2]
    ]);

    static getAllEntries() {
        return ListLength.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return ListLength.#objectValues[arguments[1]];
        }

        if (value instanceof ListLength) {
            return value;
        }

        let intVal = ListLength.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return ListLength.#objectValues[intVal];
        }

        throw TypeError(value + " is not a ListLength and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new ListLength(value);
    }

    get value(){
        return [...ListLength.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new ListLength(internalConstructor, internalConstructor, 0),
        new ListLength(internalConstructor, internalConstructor, 1),
        new ListLength(internalConstructor, internalConstructor, 2),
    ];

    static Wide = ListLength.#objectValues[0];
    static Short = ListLength.#objectValues[1];
    static Narrow = ListLength.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ListFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `TransformResult`](https://docs.rs/icu/2.1.1/icu/locale/enum.TransformResult.html) for more information.
 */
class TransformResult {
    #value = undefined;

    static #values = new Map([
        ["Modified", 0],
        ["Unmodified", 1]
    ]);

    static getAllEntries() {
        return TransformResult.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return TransformResult.#objectValues[arguments[1]];
        }

        if (value instanceof TransformResult) {
            return value;
        }

        let intVal = TransformResult.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return TransformResult.#objectValues[intVal];
        }

        throw TypeError(value + " is not a TransformResult and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new TransformResult(value);
    }

    get value(){
        return [...TransformResult.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new TransformResult(internalConstructor, internalConstructor, 0),
        new TransformResult(internalConstructor, internalConstructor, 1),
    ];

    static Modified = TransformResult.#objectValues[0];
    static Unmodified = TransformResult.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleCanonicalizer_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleExpander_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Direction`](https://docs.rs/icu/2.1.1/icu/locale/enum.Direction.html) for more information.
 */
class LocaleDirection {
    #value = undefined;

    static #values = new Map([
        ["LeftToRight", 0],
        ["RightToLeft", 1],
        ["Unknown", 2]
    ]);

    static getAllEntries() {
        return LocaleDirection.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LocaleDirection.#objectValues[arguments[1]];
        }

        if (value instanceof LocaleDirection) {
            return value;
        }

        let intVal = LocaleDirection.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LocaleDirection.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LocaleDirection and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LocaleDirection(value);
    }

    get value(){
        return [...LocaleDirection.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LocaleDirection(internalConstructor, internalConstructor, 0),
        new LocaleDirection(internalConstructor, internalConstructor, 1),
        new LocaleDirection(internalConstructor, internalConstructor, 2),
    ];

    static LeftToRight = LocaleDirection.#objectValues[0];
    static RightToLeft = LocaleDirection.#objectValues[1];
    static Unknown = LocaleDirection.#objectValues[2];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LocaleDirectionality_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_Logger_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ComposingNormalizer_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_DecomposingNormalizer_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CanonicalCombiningClassMap_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CanonicalComposition_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CanonicalDecomposition_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_PluralOperands_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `PluralCategory`](https://docs.rs/icu/2.1.1/icu/plurals/enum.PluralCategory.html) for more information.
 */
class PluralCategory {
    #value = undefined;

    static #values = new Map([
        ["Zero", 0],
        ["One", 1],
        ["Two", 2],
        ["Few", 3],
        ["Many", 4],
        ["Other", 5]
    ]);

    static getAllEntries() {
        return PluralCategory.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return PluralCategory.#objectValues[arguments[1]];
        }

        if (value instanceof PluralCategory) {
            return value;
        }

        let intVal = PluralCategory.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return PluralCategory.#objectValues[intVal];
        }

        throw TypeError(value + " is not a PluralCategory and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new PluralCategory(value);
    }

    get value(){
        return [...PluralCategory.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new PluralCategory(internalConstructor, internalConstructor, 0),
        new PluralCategory(internalConstructor, internalConstructor, 1),
        new PluralCategory(internalConstructor, internalConstructor, 2),
        new PluralCategory(internalConstructor, internalConstructor, 3),
        new PluralCategory(internalConstructor, internalConstructor, 4),
        new PluralCategory(internalConstructor, internalConstructor, 5),
    ];

    static Zero = PluralCategory.#objectValues[0];
    static One = PluralCategory.#objectValues[1];
    static Two = PluralCategory.#objectValues[2];
    static Few = PluralCategory.#objectValues[3];
    static Many = PluralCategory.#objectValues[4];
    static Other = PluralCategory.#objectValues[5];


    /**
     * Construct from a string in the format
     * [specified in TR35](https://unicode.org/reports/tr35/tr35-numbers.html#Language_Plural_Rules)
     *
     * See the [Rust documentation for `get_for_cldr_string`](https://docs.rs/icu/2.1.1/icu/plurals/enum.PluralCategory.html#method.get_for_cldr_string) for more information.
     *
     * See the [Rust documentation for `get_for_cldr_bytes`](https://docs.rs/icu/2.1.1/icu/plurals/enum.PluralCategory.html#method.get_for_cldr_bytes) for more information.
     */
    static getForCldrString(s) {
        let functionCleanupArena = new CleanupArena();

        const sSlice = functionCleanupArena.alloc(DiplomatBuf.sliceWrapper(wasm$1, DiplomatBuf.str8(wasm$1, s)));
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_PluralCategory_get_for_cldr_string_mv1(diplomatReceive.buffer, sSlice.ptr);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new PluralCategory(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            functionCleanupArena.free();

            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_PluralRules_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CodePointMapData16_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_CodePointMapData8_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_GeneralCategoryNameToGroupMapper_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_PropertyValueNameToEnumMapper_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_EmojiSetData_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ScriptExtensionsSet_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ScriptWithExtensionsBorrowed_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ScriptWithExtensions_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_GraphemeClusterBreakIteratorLatin1_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_GraphemeClusterBreakIteratorUtf16_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_GraphemeClusterBreakIteratorUtf8_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_GraphemeClusterSegmenter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LineBreakIteratorLatin1_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LineBreakIteratorUtf16_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LineBreakIteratorUtf8_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_LineSegmenter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_SentenceBreakIteratorLatin1_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_SentenceBreakIteratorUtf16_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_SentenceBreakIteratorUtf8_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_SentenceSegmenter_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `WordType`](https://docs.rs/icu/2.1.1/icu/segmenter/options/enum.WordType.html) for more information.
 */
class SegmenterWordType {
    #value = undefined;

    static #values = new Map([
        ["None", 0],
        ["Number", 1],
        ["Letter", 2]
    ]);

    static getAllEntries() {
        return SegmenterWordType.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return SegmenterWordType.#objectValues[arguments[1]];
        }

        if (value instanceof SegmenterWordType) {
            return value;
        }

        let intVal = SegmenterWordType.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return SegmenterWordType.#objectValues[intVal];
        }

        throw TypeError(value + " is not a SegmenterWordType and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new SegmenterWordType(value);
    }

    get value(){
        return [...SegmenterWordType.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new SegmenterWordType(internalConstructor, internalConstructor, 0),
        new SegmenterWordType(internalConstructor, internalConstructor, 1),
        new SegmenterWordType(internalConstructor, internalConstructor, 2),
    ];

    static None = SegmenterWordType.#objectValues[0];
    static Number = SegmenterWordType.#objectValues[1];
    static Letter = SegmenterWordType.#objectValues[2];


    /**
     * See the [Rust documentation for `is_word_like`](https://docs.rs/icu/2.1.1/icu/segmenter/options/enum.WordType.html#method.is_word_like) for more information.
     */
    get isWordLike() {

        const result = wasm$1.icu4x_SegmenterWordType_is_word_like_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WordBreakIteratorLatin1_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WordBreakIteratorUtf16_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WordBreakIteratorUtf8_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WordSegmenter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * An error when formatting a datetime.
 *
 * Currently never returned by any API.
 *
 * Additional information: [1](https://docs.rs/icu/2.1.1/icu/datetime/unchecked/enum.FormattedDateTimeUncheckedError.html)
 */
class DateTimeWriteError {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["MissingTimeZoneVariant", 1]
    ]);

    static getAllEntries() {
        return DateTimeWriteError.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return DateTimeWriteError.#objectValues[arguments[1]];
        }

        if (value instanceof DateTimeWriteError) {
            return value;
        }

        let intVal = DateTimeWriteError.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return DateTimeWriteError.#objectValues[intVal];
        }

        throw TypeError(value + " is not a DateTimeWriteError and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new DateTimeWriteError(value);
    }

    get value(){
        return [...DateTimeWriteError.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new DateTimeWriteError(internalConstructor, internalConstructor, 0),
        new DateTimeWriteError(internalConstructor, internalConstructor, 1),
    ];

    static Unknown = DateTimeWriteError.#objectValues[0];
    static MissingTimeZoneVariant = DateTimeWriteError.#objectValues[1];


    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_TimeZoneFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WeekdaySetIterator_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WeekInformation_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_WindowsParser_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ZonedDateFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ZonedDateFormatterGregorian_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ZonedDateTimeFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ZonedDateTimeFormatterGregorian_destroy_mv1(ptr);
});

// generated by diplomat-tool

new FinalizationRegistry((ptr) => {
    wasm$1.icu4x_ZonedTimeFormatter_destroy_mv1(ptr);
});

// generated by diplomat-tool



/**
 * See the [Rust documentation for `BidiClass`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.BidiClass.html) for more information.
 */
class BidiClass {
    #value = undefined;

    static #values = new Map([
        ["LeftToRight", 0],
        ["RightToLeft", 1],
        ["EuropeanNumber", 2],
        ["EuropeanSeparator", 3],
        ["EuropeanTerminator", 4],
        ["ArabicNumber", 5],
        ["CommonSeparator", 6],
        ["ParagraphSeparator", 7],
        ["SegmentSeparator", 8],
        ["WhiteSpace", 9],
        ["OtherNeutral", 10],
        ["LeftToRightEmbedding", 11],
        ["LeftToRightOverride", 12],
        ["ArabicLetter", 13],
        ["RightToLeftEmbedding", 14],
        ["RightToLeftOverride", 15],
        ["PopDirectionalFormat", 16],
        ["NonspacingMark", 17],
        ["BoundaryNeutral", 18],
        ["FirstStrongIsolate", 19],
        ["LeftToRightIsolate", 20],
        ["RightToLeftIsolate", 21],
        ["PopDirectionalIsolate", 22]
    ]);

    static getAllEntries() {
        return BidiClass.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return BidiClass.#objectValues[arguments[1]];
        }

        if (value instanceof BidiClass) {
            return value;
        }

        let intVal = BidiClass.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return BidiClass.#objectValues[intVal];
        }

        throw TypeError(value + " is not a BidiClass and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new BidiClass(value);
    }

    get value(){
        return [...BidiClass.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new BidiClass(internalConstructor, internalConstructor, 0),
        new BidiClass(internalConstructor, internalConstructor, 1),
        new BidiClass(internalConstructor, internalConstructor, 2),
        new BidiClass(internalConstructor, internalConstructor, 3),
        new BidiClass(internalConstructor, internalConstructor, 4),
        new BidiClass(internalConstructor, internalConstructor, 5),
        new BidiClass(internalConstructor, internalConstructor, 6),
        new BidiClass(internalConstructor, internalConstructor, 7),
        new BidiClass(internalConstructor, internalConstructor, 8),
        new BidiClass(internalConstructor, internalConstructor, 9),
        new BidiClass(internalConstructor, internalConstructor, 10),
        new BidiClass(internalConstructor, internalConstructor, 11),
        new BidiClass(internalConstructor, internalConstructor, 12),
        new BidiClass(internalConstructor, internalConstructor, 13),
        new BidiClass(internalConstructor, internalConstructor, 14),
        new BidiClass(internalConstructor, internalConstructor, 15),
        new BidiClass(internalConstructor, internalConstructor, 16),
        new BidiClass(internalConstructor, internalConstructor, 17),
        new BidiClass(internalConstructor, internalConstructor, 18),
        new BidiClass(internalConstructor, internalConstructor, 19),
        new BidiClass(internalConstructor, internalConstructor, 20),
        new BidiClass(internalConstructor, internalConstructor, 21),
        new BidiClass(internalConstructor, internalConstructor, 22),
    ];

    static LeftToRight = BidiClass.#objectValues[0];
    static RightToLeft = BidiClass.#objectValues[1];
    static EuropeanNumber = BidiClass.#objectValues[2];
    static EuropeanSeparator = BidiClass.#objectValues[3];
    static EuropeanTerminator = BidiClass.#objectValues[4];
    static ArabicNumber = BidiClass.#objectValues[5];
    static CommonSeparator = BidiClass.#objectValues[6];
    static ParagraphSeparator = BidiClass.#objectValues[7];
    static SegmentSeparator = BidiClass.#objectValues[8];
    static WhiteSpace = BidiClass.#objectValues[9];
    static OtherNeutral = BidiClass.#objectValues[10];
    static LeftToRightEmbedding = BidiClass.#objectValues[11];
    static LeftToRightOverride = BidiClass.#objectValues[12];
    static ArabicLetter = BidiClass.#objectValues[13];
    static RightToLeftEmbedding = BidiClass.#objectValues[14];
    static RightToLeftOverride = BidiClass.#objectValues[15];
    static PopDirectionalFormat = BidiClass.#objectValues[16];
    static NonspacingMark = BidiClass.#objectValues[17];
    static BoundaryNeutral = BidiClass.#objectValues[18];
    static FirstStrongIsolate = BidiClass.#objectValues[19];
    static LeftToRightIsolate = BidiClass.#objectValues[20];
    static RightToLeftIsolate = BidiClass.#objectValues[21];
    static PopDirectionalIsolate = BidiClass.#objectValues[22];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_BidiClass_for_char_mv1(ch);

        try {
            return new BidiClass(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_BidiClass_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_BidiClass_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.BidiClass.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_BidiClass_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.BidiClass.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_BidiClass_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new BidiClass(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `CanonicalCombiningClass`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.CanonicalCombiningClass.html) for more information.
 */
class CanonicalCombiningClass {
    #value = undefined;

    static #values = new Map([
        ["NotReordered", 0],
        ["Overlay", 1],
        ["HanReading", 6],
        ["Nukta", 7],
        ["KanaVoicing", 8],
        ["Virama", 9],
        ["Ccc10", 10],
        ["Ccc11", 11],
        ["Ccc12", 12],
        ["Ccc13", 13],
        ["Ccc14", 14],
        ["Ccc15", 15],
        ["Ccc16", 16],
        ["Ccc17", 17],
        ["Ccc18", 18],
        ["Ccc19", 19],
        ["Ccc20", 20],
        ["Ccc21", 21],
        ["Ccc22", 22],
        ["Ccc23", 23],
        ["Ccc24", 24],
        ["Ccc25", 25],
        ["Ccc26", 26],
        ["Ccc27", 27],
        ["Ccc28", 28],
        ["Ccc29", 29],
        ["Ccc30", 30],
        ["Ccc31", 31],
        ["Ccc32", 32],
        ["Ccc33", 33],
        ["Ccc34", 34],
        ["Ccc35", 35],
        ["Ccc36", 36],
        ["Ccc84", 84],
        ["Ccc91", 91],
        ["Ccc103", 103],
        ["Ccc107", 107],
        ["Ccc118", 118],
        ["Ccc122", 122],
        ["Ccc129", 129],
        ["Ccc130", 130],
        ["Ccc132", 132],
        ["Ccc133", 133],
        ["AttachedBelowLeft", 200],
        ["AttachedBelow", 202],
        ["AttachedAbove", 214],
        ["AttachedAboveRight", 216],
        ["BelowLeft", 218],
        ["Below", 220],
        ["BelowRight", 222],
        ["Left", 224],
        ["Right", 226],
        ["AboveLeft", 228],
        ["Above", 230],
        ["AboveRight", 232],
        ["DoubleBelow", 233],
        ["DoubleAbove", 234],
        ["IotaSubscript", 240]
    ]);

    static getAllEntries() {
        return CanonicalCombiningClass.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return CanonicalCombiningClass.#objectValues[arguments[1]];
        }

        if (value instanceof CanonicalCombiningClass) {
            return value;
        }

        let intVal = CanonicalCombiningClass.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return CanonicalCombiningClass.#objectValues[intVal];
        }

        throw TypeError(value + " is not a CanonicalCombiningClass and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new CanonicalCombiningClass(value);
    }

    get value(){
        for (let entry of CanonicalCombiningClass.#values) {
            if (entry[1] == this.#value) {
                return entry[0];
            }
        }
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = {
        [0]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 0),
        [1]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 1),
        [6]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 6),
        [7]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 7),
        [8]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 8),
        [9]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 9),
        [10]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 10),
        [11]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 11),
        [12]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 12),
        [13]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 13),
        [14]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 14),
        [15]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 15),
        [16]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 16),
        [17]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 17),
        [18]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 18),
        [19]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 19),
        [20]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 20),
        [21]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 21),
        [22]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 22),
        [23]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 23),
        [24]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 24),
        [25]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 25),
        [26]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 26),
        [27]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 27),
        [28]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 28),
        [29]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 29),
        [30]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 30),
        [31]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 31),
        [32]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 32),
        [33]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 33),
        [34]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 34),
        [35]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 35),
        [36]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 36),
        [84]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 84),
        [91]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 91),
        [103]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 103),
        [107]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 107),
        [118]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 118),
        [122]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 122),
        [129]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 129),
        [130]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 130),
        [132]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 132),
        [133]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 133),
        [200]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 200),
        [202]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 202),
        [214]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 214),
        [216]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 216),
        [218]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 218),
        [220]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 220),
        [222]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 222),
        [224]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 224),
        [226]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 226),
        [228]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 228),
        [230]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 230),
        [232]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 232),
        [233]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 233),
        [234]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 234),
        [240]: new CanonicalCombiningClass(internalConstructor, internalConstructor, 240),
    };

    static NotReordered = CanonicalCombiningClass.#objectValues[0];
    static Overlay = CanonicalCombiningClass.#objectValues[1];
    static HanReading = CanonicalCombiningClass.#objectValues[6];
    static Nukta = CanonicalCombiningClass.#objectValues[7];
    static KanaVoicing = CanonicalCombiningClass.#objectValues[8];
    static Virama = CanonicalCombiningClass.#objectValues[9];
    static Ccc10 = CanonicalCombiningClass.#objectValues[10];
    static Ccc11 = CanonicalCombiningClass.#objectValues[11];
    static Ccc12 = CanonicalCombiningClass.#objectValues[12];
    static Ccc13 = CanonicalCombiningClass.#objectValues[13];
    static Ccc14 = CanonicalCombiningClass.#objectValues[14];
    static Ccc15 = CanonicalCombiningClass.#objectValues[15];
    static Ccc16 = CanonicalCombiningClass.#objectValues[16];
    static Ccc17 = CanonicalCombiningClass.#objectValues[17];
    static Ccc18 = CanonicalCombiningClass.#objectValues[18];
    static Ccc19 = CanonicalCombiningClass.#objectValues[19];
    static Ccc20 = CanonicalCombiningClass.#objectValues[20];
    static Ccc21 = CanonicalCombiningClass.#objectValues[21];
    static Ccc22 = CanonicalCombiningClass.#objectValues[22];
    static Ccc23 = CanonicalCombiningClass.#objectValues[23];
    static Ccc24 = CanonicalCombiningClass.#objectValues[24];
    static Ccc25 = CanonicalCombiningClass.#objectValues[25];
    static Ccc26 = CanonicalCombiningClass.#objectValues[26];
    static Ccc27 = CanonicalCombiningClass.#objectValues[27];
    static Ccc28 = CanonicalCombiningClass.#objectValues[28];
    static Ccc29 = CanonicalCombiningClass.#objectValues[29];
    static Ccc30 = CanonicalCombiningClass.#objectValues[30];
    static Ccc31 = CanonicalCombiningClass.#objectValues[31];
    static Ccc32 = CanonicalCombiningClass.#objectValues[32];
    static Ccc33 = CanonicalCombiningClass.#objectValues[33];
    static Ccc34 = CanonicalCombiningClass.#objectValues[34];
    static Ccc35 = CanonicalCombiningClass.#objectValues[35];
    static Ccc36 = CanonicalCombiningClass.#objectValues[36];
    static Ccc84 = CanonicalCombiningClass.#objectValues[84];
    static Ccc91 = CanonicalCombiningClass.#objectValues[91];
    static Ccc103 = CanonicalCombiningClass.#objectValues[103];
    static Ccc107 = CanonicalCombiningClass.#objectValues[107];
    static Ccc118 = CanonicalCombiningClass.#objectValues[118];
    static Ccc122 = CanonicalCombiningClass.#objectValues[122];
    static Ccc129 = CanonicalCombiningClass.#objectValues[129];
    static Ccc130 = CanonicalCombiningClass.#objectValues[130];
    static Ccc132 = CanonicalCombiningClass.#objectValues[132];
    static Ccc133 = CanonicalCombiningClass.#objectValues[133];
    static AttachedBelowLeft = CanonicalCombiningClass.#objectValues[200];
    static AttachedBelow = CanonicalCombiningClass.#objectValues[202];
    static AttachedAbove = CanonicalCombiningClass.#objectValues[214];
    static AttachedAboveRight = CanonicalCombiningClass.#objectValues[216];
    static BelowLeft = CanonicalCombiningClass.#objectValues[218];
    static Below = CanonicalCombiningClass.#objectValues[220];
    static BelowRight = CanonicalCombiningClass.#objectValues[222];
    static Left = CanonicalCombiningClass.#objectValues[224];
    static Right = CanonicalCombiningClass.#objectValues[226];
    static AboveLeft = CanonicalCombiningClass.#objectValues[228];
    static Above = CanonicalCombiningClass.#objectValues[230];
    static AboveRight = CanonicalCombiningClass.#objectValues[232];
    static DoubleBelow = CanonicalCombiningClass.#objectValues[233];
    static DoubleAbove = CanonicalCombiningClass.#objectValues[234];
    static IotaSubscript = CanonicalCombiningClass.#objectValues[240];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_CanonicalCombiningClass_for_char_mv1(ch);

        try {
            return new CanonicalCombiningClass(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.CanonicalCombiningClass.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_CanonicalCombiningClass_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.CanonicalCombiningClass.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_CanonicalCombiningClass_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new CanonicalCombiningClass(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `EastAsianWidth`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.EastAsianWidth.html) for more information.
 */
class EastAsianWidth {
    #value = undefined;

    static #values = new Map([
        ["Neutral", 0],
        ["Ambiguous", 1],
        ["Halfwidth", 2],
        ["Fullwidth", 3],
        ["Narrow", 4],
        ["Wide", 5]
    ]);

    static getAllEntries() {
        return EastAsianWidth.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return EastAsianWidth.#objectValues[arguments[1]];
        }

        if (value instanceof EastAsianWidth) {
            return value;
        }

        let intVal = EastAsianWidth.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return EastAsianWidth.#objectValues[intVal];
        }

        throw TypeError(value + " is not a EastAsianWidth and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new EastAsianWidth(value);
    }

    get value(){
        return [...EastAsianWidth.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new EastAsianWidth(internalConstructor, internalConstructor, 0),
        new EastAsianWidth(internalConstructor, internalConstructor, 1),
        new EastAsianWidth(internalConstructor, internalConstructor, 2),
        new EastAsianWidth(internalConstructor, internalConstructor, 3),
        new EastAsianWidth(internalConstructor, internalConstructor, 4),
        new EastAsianWidth(internalConstructor, internalConstructor, 5),
    ];

    static Neutral = EastAsianWidth.#objectValues[0];
    static Ambiguous = EastAsianWidth.#objectValues[1];
    static Halfwidth = EastAsianWidth.#objectValues[2];
    static Fullwidth = EastAsianWidth.#objectValues[3];
    static Narrow = EastAsianWidth.#objectValues[4];
    static Wide = EastAsianWidth.#objectValues[5];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_EastAsianWidth_for_char_mv1(ch);

        try {
            return new EastAsianWidth(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_EastAsianWidth_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_EastAsianWidth_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.EastAsianWidth.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_EastAsianWidth_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.EastAsianWidth.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_EastAsianWidth_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new EastAsianWidth(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `GraphemeClusterBreak`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GraphemeClusterBreak.html) for more information.
 */
class GraphemeClusterBreak {
    #value = undefined;

    static #values = new Map([
        ["Other", 0],
        ["Control", 1],
        ["Cr", 2],
        ["Extend", 3],
        ["L", 4],
        ["Lf", 5],
        ["Lv", 6],
        ["Lvt", 7],
        ["T", 8],
        ["V", 9],
        ["SpacingMark", 10],
        ["Prepend", 11],
        ["RegionalIndicator", 12],
        ["EBase", 13],
        ["EBaseGaz", 14],
        ["EModifier", 15],
        ["GlueAfterZwj", 16],
        ["Zwj", 17]
    ]);

    static getAllEntries() {
        return GraphemeClusterBreak.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return GraphemeClusterBreak.#objectValues[arguments[1]];
        }

        if (value instanceof GraphemeClusterBreak) {
            return value;
        }

        let intVal = GraphemeClusterBreak.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return GraphemeClusterBreak.#objectValues[intVal];
        }

        throw TypeError(value + " is not a GraphemeClusterBreak and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new GraphemeClusterBreak(value);
    }

    get value(){
        return [...GraphemeClusterBreak.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 0),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 1),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 2),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 3),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 4),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 5),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 6),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 7),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 8),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 9),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 10),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 11),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 12),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 13),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 14),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 15),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 16),
        new GraphemeClusterBreak(internalConstructor, internalConstructor, 17),
    ];

    static Other = GraphemeClusterBreak.#objectValues[0];
    static Control = GraphemeClusterBreak.#objectValues[1];
    static Cr = GraphemeClusterBreak.#objectValues[2];
    static Extend = GraphemeClusterBreak.#objectValues[3];
    static L = GraphemeClusterBreak.#objectValues[4];
    static Lf = GraphemeClusterBreak.#objectValues[5];
    static Lv = GraphemeClusterBreak.#objectValues[6];
    static Lvt = GraphemeClusterBreak.#objectValues[7];
    static T = GraphemeClusterBreak.#objectValues[8];
    static V = GraphemeClusterBreak.#objectValues[9];
    static SpacingMark = GraphemeClusterBreak.#objectValues[10];
    static Prepend = GraphemeClusterBreak.#objectValues[11];
    static RegionalIndicator = GraphemeClusterBreak.#objectValues[12];
    static EBase = GraphemeClusterBreak.#objectValues[13];
    static EBaseGaz = GraphemeClusterBreak.#objectValues[14];
    static EModifier = GraphemeClusterBreak.#objectValues[15];
    static GlueAfterZwj = GraphemeClusterBreak.#objectValues[16];
    static Zwj = GraphemeClusterBreak.#objectValues[17];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_GraphemeClusterBreak_for_char_mv1(ch);

        try {
            return new GraphemeClusterBreak(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GraphemeClusterBreak.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_GraphemeClusterBreak_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.GraphemeClusterBreak.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_GraphemeClusterBreak_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new GraphemeClusterBreak(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `HangulSyllableType`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.HangulSyllableType.html) for more information.
 */
class HangulSyllableType {
    #value = undefined;

    static #values = new Map([
        ["NotApplicable", 0],
        ["LeadingJamo", 1],
        ["VowelJamo", 2],
        ["TrailingJamo", 3],
        ["LeadingVowelSyllable", 4],
        ["LeadingVowelTrailingSyllable", 5]
    ]);

    static getAllEntries() {
        return HangulSyllableType.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return HangulSyllableType.#objectValues[arguments[1]];
        }

        if (value instanceof HangulSyllableType) {
            return value;
        }

        let intVal = HangulSyllableType.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return HangulSyllableType.#objectValues[intVal];
        }

        throw TypeError(value + " is not a HangulSyllableType and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new HangulSyllableType(value);
    }

    get value(){
        return [...HangulSyllableType.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new HangulSyllableType(internalConstructor, internalConstructor, 0),
        new HangulSyllableType(internalConstructor, internalConstructor, 1),
        new HangulSyllableType(internalConstructor, internalConstructor, 2),
        new HangulSyllableType(internalConstructor, internalConstructor, 3),
        new HangulSyllableType(internalConstructor, internalConstructor, 4),
        new HangulSyllableType(internalConstructor, internalConstructor, 5),
    ];

    static NotApplicable = HangulSyllableType.#objectValues[0];
    static LeadingJamo = HangulSyllableType.#objectValues[1];
    static VowelJamo = HangulSyllableType.#objectValues[2];
    static TrailingJamo = HangulSyllableType.#objectValues[3];
    static LeadingVowelSyllable = HangulSyllableType.#objectValues[4];
    static LeadingVowelTrailingSyllable = HangulSyllableType.#objectValues[5];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_HangulSyllableType_for_char_mv1(ch);

        try {
            return new HangulSyllableType(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.HangulSyllableType.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_HangulSyllableType_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.HangulSyllableType.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_HangulSyllableType_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new HangulSyllableType(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `IndicSyllabicCategory`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.IndicSyllabicCategory.html) for more information.
 */
class IndicSyllabicCategory {
    #value = undefined;

    static #values = new Map([
        ["Other", 0],
        ["Avagraha", 1],
        ["Bindu", 2],
        ["BrahmiJoiningNumber", 3],
        ["CantillationMark", 4],
        ["Consonant", 5],
        ["ConsonantDead", 6],
        ["ConsonantFinal", 7],
        ["ConsonantHeadLetter", 8],
        ["ConsonantInitialPostfixed", 9],
        ["ConsonantKiller", 10],
        ["ConsonantMedial", 11],
        ["ConsonantPlaceholder", 12],
        ["ConsonantPrecedingRepha", 13],
        ["ConsonantPrefixed", 14],
        ["ConsonantSucceedingRepha", 15],
        ["ConsonantSubjoined", 16],
        ["ConsonantWithStacker", 17],
        ["GeminationMark", 18],
        ["InvisibleStacker", 19],
        ["Joiner", 20],
        ["ModifyingLetter", 21],
        ["NonJoiner", 22],
        ["Nukta", 23],
        ["Number", 24],
        ["NumberJoiner", 25],
        ["PureKiller", 26],
        ["RegisterShifter", 27],
        ["SyllableModifier", 28],
        ["ToneLetter", 29],
        ["ToneMark", 30],
        ["Virama", 31],
        ["Visarga", 32],
        ["Vowel", 33],
        ["VowelDependent", 34],
        ["VowelIndependent", 35],
        ["ReorderingKiller", 36]
    ]);

    static getAllEntries() {
        return IndicSyllabicCategory.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return IndicSyllabicCategory.#objectValues[arguments[1]];
        }

        if (value instanceof IndicSyllabicCategory) {
            return value;
        }

        let intVal = IndicSyllabicCategory.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return IndicSyllabicCategory.#objectValues[intVal];
        }

        throw TypeError(value + " is not a IndicSyllabicCategory and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new IndicSyllabicCategory(value);
    }

    get value(){
        return [...IndicSyllabicCategory.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 0),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 1),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 2),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 3),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 4),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 5),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 6),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 7),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 8),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 9),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 10),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 11),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 12),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 13),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 14),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 15),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 16),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 17),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 18),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 19),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 20),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 21),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 22),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 23),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 24),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 25),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 26),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 27),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 28),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 29),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 30),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 31),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 32),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 33),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 34),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 35),
        new IndicSyllabicCategory(internalConstructor, internalConstructor, 36),
    ];

    static Other = IndicSyllabicCategory.#objectValues[0];
    static Avagraha = IndicSyllabicCategory.#objectValues[1];
    static Bindu = IndicSyllabicCategory.#objectValues[2];
    static BrahmiJoiningNumber = IndicSyllabicCategory.#objectValues[3];
    static CantillationMark = IndicSyllabicCategory.#objectValues[4];
    static Consonant = IndicSyllabicCategory.#objectValues[5];
    static ConsonantDead = IndicSyllabicCategory.#objectValues[6];
    static ConsonantFinal = IndicSyllabicCategory.#objectValues[7];
    static ConsonantHeadLetter = IndicSyllabicCategory.#objectValues[8];
    static ConsonantInitialPostfixed = IndicSyllabicCategory.#objectValues[9];
    static ConsonantKiller = IndicSyllabicCategory.#objectValues[10];
    static ConsonantMedial = IndicSyllabicCategory.#objectValues[11];
    static ConsonantPlaceholder = IndicSyllabicCategory.#objectValues[12];
    static ConsonantPrecedingRepha = IndicSyllabicCategory.#objectValues[13];
    static ConsonantPrefixed = IndicSyllabicCategory.#objectValues[14];
    static ConsonantSucceedingRepha = IndicSyllabicCategory.#objectValues[15];
    static ConsonantSubjoined = IndicSyllabicCategory.#objectValues[16];
    static ConsonantWithStacker = IndicSyllabicCategory.#objectValues[17];
    static GeminationMark = IndicSyllabicCategory.#objectValues[18];
    static InvisibleStacker = IndicSyllabicCategory.#objectValues[19];
    static Joiner = IndicSyllabicCategory.#objectValues[20];
    static ModifyingLetter = IndicSyllabicCategory.#objectValues[21];
    static NonJoiner = IndicSyllabicCategory.#objectValues[22];
    static Nukta = IndicSyllabicCategory.#objectValues[23];
    static Number = IndicSyllabicCategory.#objectValues[24];
    static NumberJoiner = IndicSyllabicCategory.#objectValues[25];
    static PureKiller = IndicSyllabicCategory.#objectValues[26];
    static RegisterShifter = IndicSyllabicCategory.#objectValues[27];
    static SyllableModifier = IndicSyllabicCategory.#objectValues[28];
    static ToneLetter = IndicSyllabicCategory.#objectValues[29];
    static ToneMark = IndicSyllabicCategory.#objectValues[30];
    static Virama = IndicSyllabicCategory.#objectValues[31];
    static Visarga = IndicSyllabicCategory.#objectValues[32];
    static Vowel = IndicSyllabicCategory.#objectValues[33];
    static VowelDependent = IndicSyllabicCategory.#objectValues[34];
    static VowelIndependent = IndicSyllabicCategory.#objectValues[35];
    static ReorderingKiller = IndicSyllabicCategory.#objectValues[36];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_IndicSyllabicCategory_for_char_mv1(ch);

        try {
            return new IndicSyllabicCategory(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.IndicSyllabicCategory.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_IndicSyllabicCategory_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.IndicSyllabicCategory.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_IndicSyllabicCategory_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new IndicSyllabicCategory(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `JoiningType`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.JoiningType.html) for more information.
 */
class JoiningType {
    #value = undefined;

    static #values = new Map([
        ["NonJoining", 0],
        ["JoinCausing", 1],
        ["DualJoining", 2],
        ["LeftJoining", 3],
        ["RightJoining", 4],
        ["Transparent", 5]
    ]);

    static getAllEntries() {
        return JoiningType.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return JoiningType.#objectValues[arguments[1]];
        }

        if (value instanceof JoiningType) {
            return value;
        }

        let intVal = JoiningType.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return JoiningType.#objectValues[intVal];
        }

        throw TypeError(value + " is not a JoiningType and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new JoiningType(value);
    }

    get value(){
        return [...JoiningType.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new JoiningType(internalConstructor, internalConstructor, 0),
        new JoiningType(internalConstructor, internalConstructor, 1),
        new JoiningType(internalConstructor, internalConstructor, 2),
        new JoiningType(internalConstructor, internalConstructor, 3),
        new JoiningType(internalConstructor, internalConstructor, 4),
        new JoiningType(internalConstructor, internalConstructor, 5),
    ];

    static NonJoining = JoiningType.#objectValues[0];
    static JoinCausing = JoiningType.#objectValues[1];
    static DualJoining = JoiningType.#objectValues[2];
    static LeftJoining = JoiningType.#objectValues[3];
    static RightJoining = JoiningType.#objectValues[4];
    static Transparent = JoiningType.#objectValues[5];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_JoiningType_for_char_mv1(ch);

        try {
            return new JoiningType(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_JoiningType_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_JoiningType_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.JoiningType.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_JoiningType_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.JoiningType.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_JoiningType_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new JoiningType(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `LineBreak`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.LineBreak.html) for more information.
 */
class LineBreak {
    #value = undefined;

    static #values = new Map([
        ["Unknown", 0],
        ["Ambiguous", 1],
        ["Alphabetic", 2],
        ["BreakBoth", 3],
        ["BreakAfter", 4],
        ["BreakBefore", 5],
        ["MandatoryBreak", 6],
        ["ContingentBreak", 7],
        ["ClosePunctuation", 8],
        ["CombiningMark", 9],
        ["CarriageReturn", 10],
        ["Exclamation", 11],
        ["Glue", 12],
        ["Hyphen", 13],
        ["Ideographic", 14],
        ["Inseparable", 15],
        ["InfixNumeric", 16],
        ["LineFeed", 17],
        ["Nonstarter", 18],
        ["Numeric", 19],
        ["OpenPunctuation", 20],
        ["PostfixNumeric", 21],
        ["PrefixNumeric", 22],
        ["Quotation", 23],
        ["ComplexContext", 24],
        ["Surrogate", 25],
        ["Space", 26],
        ["BreakSymbols", 27],
        ["ZwSpace", 28],
        ["NextLine", 29],
        ["WordJoiner", 30],
        ["H2", 31],
        ["H3", 32],
        ["Jl", 33],
        ["Jt", 34],
        ["Jv", 35],
        ["CloseParenthesis", 36],
        ["ConditionalJapaneseStarter", 37],
        ["HebrewLetter", 38],
        ["RegionalIndicator", 39],
        ["EBase", 40],
        ["EModifier", 41],
        ["Zwj", 42],
        ["Aksara", 43],
        ["AksaraPrebase", 44],
        ["AksaraStart", 45],
        ["ViramaFinal", 46],
        ["Virama", 47],
        ["UnambiguousHyphen", 48]
    ]);

    static getAllEntries() {
        return LineBreak.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return LineBreak.#objectValues[arguments[1]];
        }

        if (value instanceof LineBreak) {
            return value;
        }

        let intVal = LineBreak.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return LineBreak.#objectValues[intVal];
        }

        throw TypeError(value + " is not a LineBreak and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new LineBreak(value);
    }

    get value(){
        return [...LineBreak.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new LineBreak(internalConstructor, internalConstructor, 0),
        new LineBreak(internalConstructor, internalConstructor, 1),
        new LineBreak(internalConstructor, internalConstructor, 2),
        new LineBreak(internalConstructor, internalConstructor, 3),
        new LineBreak(internalConstructor, internalConstructor, 4),
        new LineBreak(internalConstructor, internalConstructor, 5),
        new LineBreak(internalConstructor, internalConstructor, 6),
        new LineBreak(internalConstructor, internalConstructor, 7),
        new LineBreak(internalConstructor, internalConstructor, 8),
        new LineBreak(internalConstructor, internalConstructor, 9),
        new LineBreak(internalConstructor, internalConstructor, 10),
        new LineBreak(internalConstructor, internalConstructor, 11),
        new LineBreak(internalConstructor, internalConstructor, 12),
        new LineBreak(internalConstructor, internalConstructor, 13),
        new LineBreak(internalConstructor, internalConstructor, 14),
        new LineBreak(internalConstructor, internalConstructor, 15),
        new LineBreak(internalConstructor, internalConstructor, 16),
        new LineBreak(internalConstructor, internalConstructor, 17),
        new LineBreak(internalConstructor, internalConstructor, 18),
        new LineBreak(internalConstructor, internalConstructor, 19),
        new LineBreak(internalConstructor, internalConstructor, 20),
        new LineBreak(internalConstructor, internalConstructor, 21),
        new LineBreak(internalConstructor, internalConstructor, 22),
        new LineBreak(internalConstructor, internalConstructor, 23),
        new LineBreak(internalConstructor, internalConstructor, 24),
        new LineBreak(internalConstructor, internalConstructor, 25),
        new LineBreak(internalConstructor, internalConstructor, 26),
        new LineBreak(internalConstructor, internalConstructor, 27),
        new LineBreak(internalConstructor, internalConstructor, 28),
        new LineBreak(internalConstructor, internalConstructor, 29),
        new LineBreak(internalConstructor, internalConstructor, 30),
        new LineBreak(internalConstructor, internalConstructor, 31),
        new LineBreak(internalConstructor, internalConstructor, 32),
        new LineBreak(internalConstructor, internalConstructor, 33),
        new LineBreak(internalConstructor, internalConstructor, 34),
        new LineBreak(internalConstructor, internalConstructor, 35),
        new LineBreak(internalConstructor, internalConstructor, 36),
        new LineBreak(internalConstructor, internalConstructor, 37),
        new LineBreak(internalConstructor, internalConstructor, 38),
        new LineBreak(internalConstructor, internalConstructor, 39),
        new LineBreak(internalConstructor, internalConstructor, 40),
        new LineBreak(internalConstructor, internalConstructor, 41),
        new LineBreak(internalConstructor, internalConstructor, 42),
        new LineBreak(internalConstructor, internalConstructor, 43),
        new LineBreak(internalConstructor, internalConstructor, 44),
        new LineBreak(internalConstructor, internalConstructor, 45),
        new LineBreak(internalConstructor, internalConstructor, 46),
        new LineBreak(internalConstructor, internalConstructor, 47),
        new LineBreak(internalConstructor, internalConstructor, 48),
    ];

    static Unknown = LineBreak.#objectValues[0];
    static Ambiguous = LineBreak.#objectValues[1];
    static Alphabetic = LineBreak.#objectValues[2];
    static BreakBoth = LineBreak.#objectValues[3];
    static BreakAfter = LineBreak.#objectValues[4];
    static BreakBefore = LineBreak.#objectValues[5];
    static MandatoryBreak = LineBreak.#objectValues[6];
    static ContingentBreak = LineBreak.#objectValues[7];
    static ClosePunctuation = LineBreak.#objectValues[8];
    static CombiningMark = LineBreak.#objectValues[9];
    static CarriageReturn = LineBreak.#objectValues[10];
    static Exclamation = LineBreak.#objectValues[11];
    static Glue = LineBreak.#objectValues[12];
    static Hyphen = LineBreak.#objectValues[13];
    static Ideographic = LineBreak.#objectValues[14];
    static Inseparable = LineBreak.#objectValues[15];
    static InfixNumeric = LineBreak.#objectValues[16];
    static LineFeed = LineBreak.#objectValues[17];
    static Nonstarter = LineBreak.#objectValues[18];
    static Numeric = LineBreak.#objectValues[19];
    static OpenPunctuation = LineBreak.#objectValues[20];
    static PostfixNumeric = LineBreak.#objectValues[21];
    static PrefixNumeric = LineBreak.#objectValues[22];
    static Quotation = LineBreak.#objectValues[23];
    static ComplexContext = LineBreak.#objectValues[24];
    static Surrogate = LineBreak.#objectValues[25];
    static Space = LineBreak.#objectValues[26];
    static BreakSymbols = LineBreak.#objectValues[27];
    static ZwSpace = LineBreak.#objectValues[28];
    static NextLine = LineBreak.#objectValues[29];
    static WordJoiner = LineBreak.#objectValues[30];
    static H2 = LineBreak.#objectValues[31];
    static H3 = LineBreak.#objectValues[32];
    static Jl = LineBreak.#objectValues[33];
    static Jt = LineBreak.#objectValues[34];
    static Jv = LineBreak.#objectValues[35];
    static CloseParenthesis = LineBreak.#objectValues[36];
    static ConditionalJapaneseStarter = LineBreak.#objectValues[37];
    static HebrewLetter = LineBreak.#objectValues[38];
    static RegionalIndicator = LineBreak.#objectValues[39];
    static EBase = LineBreak.#objectValues[40];
    static EModifier = LineBreak.#objectValues[41];
    static Zwj = LineBreak.#objectValues[42];
    static Aksara = LineBreak.#objectValues[43];
    static AksaraPrebase = LineBreak.#objectValues[44];
    static AksaraStart = LineBreak.#objectValues[45];
    static ViramaFinal = LineBreak.#objectValues[46];
    static Virama = LineBreak.#objectValues[47];
    static UnambiguousHyphen = LineBreak.#objectValues[48];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_LineBreak_for_char_mv1(ch);

        try {
            return new LineBreak(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_LineBreak_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_LineBreak_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.LineBreak.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_LineBreak_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.LineBreak.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_LineBreak_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new LineBreak(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `Script`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.Script.html) for more information.
 */
class Script {
    #value = undefined;

    static #values = new Map([
        ["Adlam", 167],
        ["Ahom", 161],
        ["AnatolianHieroglyphs", 156],
        ["Arabic", 2],
        ["Armenian", 3],
        ["Avestan", 117],
        ["Balinese", 62],
        ["Bamum", 130],
        ["BassaVah", 134],
        ["Batak", 63],
        ["Bengali", 4],
        ["BeriaErfe", 208],
        ["Bhaiksuki", 168],
        ["Bopomofo", 5],
        ["Brahmi", 65],
        ["Braille", 46],
        ["Buginese", 55],
        ["Buhid", 44],
        ["CanadianAboriginal", 40],
        ["Carian", 104],
        ["CaucasianAlbanian", 159],
        ["Chakma", 118],
        ["Cham", 66],
        ["Cherokee", 6],
        ["Chisoi", 209],
        ["Chorasmian", 189],
        ["Common", 0],
        ["Coptic", 7],
        ["Cuneiform", 101],
        ["Cypriot", 47],
        ["CyproMinoan", 193],
        ["Cyrillic", 8],
        ["Deseret", 9],
        ["Devanagari", 10],
        ["DivesAkuru", 190],
        ["Dogra", 178],
        ["Duployan", 135],
        ["EgyptianHieroglyphs", 71],
        ["Elbasan", 136],
        ["Elymaic", 185],
        ["Ethiopian", 11],
        ["Georgian", 12],
        ["Glagolitic", 56],
        ["Gothic", 13],
        ["Grantha", 137],
        ["Greek", 14],
        ["Gujarati", 15],
        ["GunjalaGondi", 179],
        ["Gurmukhi", 16],
        ["Han", 17],
        ["Hangul", 18],
        ["HanifiRohingya", 182],
        ["Hanunoo", 43],
        ["Hatran", 162],
        ["Hebrew", 19],
        ["Hiragana", 20],
        ["ImperialAramaic", 116],
        ["Inherited", 1],
        ["InscriptionalPahlavi", 122],
        ["InscriptionalParthian", 125],
        ["Javanese", 78],
        ["Kaithi", 120],
        ["Kannada", 21],
        ["Katakana", 22],
        ["Kawi", 198],
        ["KayahLi", 79],
        ["Kharoshthi", 57],
        ["KhitanSmallScript", 191],
        ["Khmer", 23],
        ["Khojki", 157],
        ["Khudawadi", 145],
        ["Lao", 24],
        ["Latin", 25],
        ["Lepcha", 82],
        ["Limbu", 48],
        ["LinearA", 83],
        ["LinearB", 49],
        ["Lisu", 131],
        ["Lycian", 107],
        ["Lydian", 108],
        ["Mahajani", 160],
        ["Makasar", 180],
        ["Malayalam", 26],
        ["Mandaic", 84],
        ["Manichaean", 121],
        ["Marchen", 169],
        ["MasaramGondi", 175],
        ["Medefaidrin", 181],
        ["MeeteiMayek", 115],
        ["MendeKikakui", 140],
        ["MeroiticCursive", 141],
        ["MeroiticHieroglyphs", 86],
        ["Miao", 92],
        ["Modi", 163],
        ["Mongolian", 27],
        ["Mro", 149],
        ["Multani", 164],
        ["Myanmar", 28],
        ["Nabataean", 143],
        ["NagMundari", 199],
        ["Nandinagari", 187],
        ["Nastaliq", 200],
        ["NewTaiLue", 59],
        ["Newa", 170],
        ["Nko", 87],
        ["Nushu", 150],
        ["NyiakengPuachueHmong", 186],
        ["Ogham", 29],
        ["OlChiki", 109],
        ["OldHungarian", 76],
        ["OldItalic", 30],
        ["OldNorthArabian", 142],
        ["OldPermic", 89],
        ["OldPersian", 61],
        ["OldSogdian", 184],
        ["OldSouthArabian", 133],
        ["OldTurkic", 88],
        ["OldUyghur", 194],
        ["Oriya", 31],
        ["Osage", 171],
        ["Osmanya", 50],
        ["PahawhHmong", 75],
        ["Palmyrene", 144],
        ["PauCinHau", 165],
        ["PhagsPa", 90],
        ["Phoenician", 91],
        ["PsalterPahlavi", 123],
        ["Rejang", 110],
        ["Runic", 32],
        ["Samaritan", 126],
        ["Saurashtra", 111],
        ["Sharada", 151],
        ["Shavian", 51],
        ["Siddham", 166],
        ["Sidetic", 210],
        ["SignWriting", 112],
        ["Sinhala", 33],
        ["Sogdian", 183],
        ["SoraSompeng", 152],
        ["Soyombo", 176],
        ["Sundanese", 113],
        ["SylotiNagri", 58],
        ["Syriac", 34],
        ["Tagalog", 42],
        ["Tagbanwa", 45],
        ["TaiLe", 52],
        ["TaiTham", 106],
        ["TaiViet", 127],
        ["TaiYo", 211],
        ["Takri", 153],
        ["Tamil", 35],
        ["Tangsa", 195],
        ["Tangut", 154],
        ["Telugu", 36],
        ["Thaana", 37],
        ["Thai", 38],
        ["Tibetan", 39],
        ["Tifinagh", 60],
        ["Tirhuta", 158],
        ["TolongSiki", 212],
        ["Toto", 196],
        ["Ugaritic", 53],
        ["Unknown", 103],
        ["Vai", 99],
        ["Vithkuqi", 197],
        ["Wancho", 188],
        ["WarangCiti", 146],
        ["Yezidi", 192],
        ["Yi", 41],
        ["ZanabazarSquare", 177]
    ]);

    static getAllEntries() {
        return Script.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return Script.#objectValues[arguments[1]];
        }

        if (value instanceof Script) {
            return value;
        }

        let intVal = Script.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return Script.#objectValues[intVal];
        }

        throw TypeError(value + " is not a Script and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new Script(value);
    }

    get value(){
        for (let entry of Script.#values) {
            if (entry[1] == this.#value) {
                return entry[0];
            }
        }
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = {
        [167]: new Script(internalConstructor, internalConstructor, 167),
        [161]: new Script(internalConstructor, internalConstructor, 161),
        [156]: new Script(internalConstructor, internalConstructor, 156),
        [2]: new Script(internalConstructor, internalConstructor, 2),
        [3]: new Script(internalConstructor, internalConstructor, 3),
        [117]: new Script(internalConstructor, internalConstructor, 117),
        [62]: new Script(internalConstructor, internalConstructor, 62),
        [130]: new Script(internalConstructor, internalConstructor, 130),
        [134]: new Script(internalConstructor, internalConstructor, 134),
        [63]: new Script(internalConstructor, internalConstructor, 63),
        [4]: new Script(internalConstructor, internalConstructor, 4),
        [208]: new Script(internalConstructor, internalConstructor, 208),
        [168]: new Script(internalConstructor, internalConstructor, 168),
        [5]: new Script(internalConstructor, internalConstructor, 5),
        [65]: new Script(internalConstructor, internalConstructor, 65),
        [46]: new Script(internalConstructor, internalConstructor, 46),
        [55]: new Script(internalConstructor, internalConstructor, 55),
        [44]: new Script(internalConstructor, internalConstructor, 44),
        [40]: new Script(internalConstructor, internalConstructor, 40),
        [104]: new Script(internalConstructor, internalConstructor, 104),
        [159]: new Script(internalConstructor, internalConstructor, 159),
        [118]: new Script(internalConstructor, internalConstructor, 118),
        [66]: new Script(internalConstructor, internalConstructor, 66),
        [6]: new Script(internalConstructor, internalConstructor, 6),
        [209]: new Script(internalConstructor, internalConstructor, 209),
        [189]: new Script(internalConstructor, internalConstructor, 189),
        [0]: new Script(internalConstructor, internalConstructor, 0),
        [7]: new Script(internalConstructor, internalConstructor, 7),
        [101]: new Script(internalConstructor, internalConstructor, 101),
        [47]: new Script(internalConstructor, internalConstructor, 47),
        [193]: new Script(internalConstructor, internalConstructor, 193),
        [8]: new Script(internalConstructor, internalConstructor, 8),
        [9]: new Script(internalConstructor, internalConstructor, 9),
        [10]: new Script(internalConstructor, internalConstructor, 10),
        [190]: new Script(internalConstructor, internalConstructor, 190),
        [178]: new Script(internalConstructor, internalConstructor, 178),
        [135]: new Script(internalConstructor, internalConstructor, 135),
        [71]: new Script(internalConstructor, internalConstructor, 71),
        [136]: new Script(internalConstructor, internalConstructor, 136),
        [185]: new Script(internalConstructor, internalConstructor, 185),
        [11]: new Script(internalConstructor, internalConstructor, 11),
        [12]: new Script(internalConstructor, internalConstructor, 12),
        [56]: new Script(internalConstructor, internalConstructor, 56),
        [13]: new Script(internalConstructor, internalConstructor, 13),
        [137]: new Script(internalConstructor, internalConstructor, 137),
        [14]: new Script(internalConstructor, internalConstructor, 14),
        [15]: new Script(internalConstructor, internalConstructor, 15),
        [179]: new Script(internalConstructor, internalConstructor, 179),
        [16]: new Script(internalConstructor, internalConstructor, 16),
        [17]: new Script(internalConstructor, internalConstructor, 17),
        [18]: new Script(internalConstructor, internalConstructor, 18),
        [182]: new Script(internalConstructor, internalConstructor, 182),
        [43]: new Script(internalConstructor, internalConstructor, 43),
        [162]: new Script(internalConstructor, internalConstructor, 162),
        [19]: new Script(internalConstructor, internalConstructor, 19),
        [20]: new Script(internalConstructor, internalConstructor, 20),
        [116]: new Script(internalConstructor, internalConstructor, 116),
        [1]: new Script(internalConstructor, internalConstructor, 1),
        [122]: new Script(internalConstructor, internalConstructor, 122),
        [125]: new Script(internalConstructor, internalConstructor, 125),
        [78]: new Script(internalConstructor, internalConstructor, 78),
        [120]: new Script(internalConstructor, internalConstructor, 120),
        [21]: new Script(internalConstructor, internalConstructor, 21),
        [22]: new Script(internalConstructor, internalConstructor, 22),
        [198]: new Script(internalConstructor, internalConstructor, 198),
        [79]: new Script(internalConstructor, internalConstructor, 79),
        [57]: new Script(internalConstructor, internalConstructor, 57),
        [191]: new Script(internalConstructor, internalConstructor, 191),
        [23]: new Script(internalConstructor, internalConstructor, 23),
        [157]: new Script(internalConstructor, internalConstructor, 157),
        [145]: new Script(internalConstructor, internalConstructor, 145),
        [24]: new Script(internalConstructor, internalConstructor, 24),
        [25]: new Script(internalConstructor, internalConstructor, 25),
        [82]: new Script(internalConstructor, internalConstructor, 82),
        [48]: new Script(internalConstructor, internalConstructor, 48),
        [83]: new Script(internalConstructor, internalConstructor, 83),
        [49]: new Script(internalConstructor, internalConstructor, 49),
        [131]: new Script(internalConstructor, internalConstructor, 131),
        [107]: new Script(internalConstructor, internalConstructor, 107),
        [108]: new Script(internalConstructor, internalConstructor, 108),
        [160]: new Script(internalConstructor, internalConstructor, 160),
        [180]: new Script(internalConstructor, internalConstructor, 180),
        [26]: new Script(internalConstructor, internalConstructor, 26),
        [84]: new Script(internalConstructor, internalConstructor, 84),
        [121]: new Script(internalConstructor, internalConstructor, 121),
        [169]: new Script(internalConstructor, internalConstructor, 169),
        [175]: new Script(internalConstructor, internalConstructor, 175),
        [181]: new Script(internalConstructor, internalConstructor, 181),
        [115]: new Script(internalConstructor, internalConstructor, 115),
        [140]: new Script(internalConstructor, internalConstructor, 140),
        [141]: new Script(internalConstructor, internalConstructor, 141),
        [86]: new Script(internalConstructor, internalConstructor, 86),
        [92]: new Script(internalConstructor, internalConstructor, 92),
        [163]: new Script(internalConstructor, internalConstructor, 163),
        [27]: new Script(internalConstructor, internalConstructor, 27),
        [149]: new Script(internalConstructor, internalConstructor, 149),
        [164]: new Script(internalConstructor, internalConstructor, 164),
        [28]: new Script(internalConstructor, internalConstructor, 28),
        [143]: new Script(internalConstructor, internalConstructor, 143),
        [199]: new Script(internalConstructor, internalConstructor, 199),
        [187]: new Script(internalConstructor, internalConstructor, 187),
        [200]: new Script(internalConstructor, internalConstructor, 200),
        [59]: new Script(internalConstructor, internalConstructor, 59),
        [170]: new Script(internalConstructor, internalConstructor, 170),
        [87]: new Script(internalConstructor, internalConstructor, 87),
        [150]: new Script(internalConstructor, internalConstructor, 150),
        [186]: new Script(internalConstructor, internalConstructor, 186),
        [29]: new Script(internalConstructor, internalConstructor, 29),
        [109]: new Script(internalConstructor, internalConstructor, 109),
        [76]: new Script(internalConstructor, internalConstructor, 76),
        [30]: new Script(internalConstructor, internalConstructor, 30),
        [142]: new Script(internalConstructor, internalConstructor, 142),
        [89]: new Script(internalConstructor, internalConstructor, 89),
        [61]: new Script(internalConstructor, internalConstructor, 61),
        [184]: new Script(internalConstructor, internalConstructor, 184),
        [133]: new Script(internalConstructor, internalConstructor, 133),
        [88]: new Script(internalConstructor, internalConstructor, 88),
        [194]: new Script(internalConstructor, internalConstructor, 194),
        [31]: new Script(internalConstructor, internalConstructor, 31),
        [171]: new Script(internalConstructor, internalConstructor, 171),
        [50]: new Script(internalConstructor, internalConstructor, 50),
        [75]: new Script(internalConstructor, internalConstructor, 75),
        [144]: new Script(internalConstructor, internalConstructor, 144),
        [165]: new Script(internalConstructor, internalConstructor, 165),
        [90]: new Script(internalConstructor, internalConstructor, 90),
        [91]: new Script(internalConstructor, internalConstructor, 91),
        [123]: new Script(internalConstructor, internalConstructor, 123),
        [110]: new Script(internalConstructor, internalConstructor, 110),
        [32]: new Script(internalConstructor, internalConstructor, 32),
        [126]: new Script(internalConstructor, internalConstructor, 126),
        [111]: new Script(internalConstructor, internalConstructor, 111),
        [151]: new Script(internalConstructor, internalConstructor, 151),
        [51]: new Script(internalConstructor, internalConstructor, 51),
        [166]: new Script(internalConstructor, internalConstructor, 166),
        [210]: new Script(internalConstructor, internalConstructor, 210),
        [112]: new Script(internalConstructor, internalConstructor, 112),
        [33]: new Script(internalConstructor, internalConstructor, 33),
        [183]: new Script(internalConstructor, internalConstructor, 183),
        [152]: new Script(internalConstructor, internalConstructor, 152),
        [176]: new Script(internalConstructor, internalConstructor, 176),
        [113]: new Script(internalConstructor, internalConstructor, 113),
        [58]: new Script(internalConstructor, internalConstructor, 58),
        [34]: new Script(internalConstructor, internalConstructor, 34),
        [42]: new Script(internalConstructor, internalConstructor, 42),
        [45]: new Script(internalConstructor, internalConstructor, 45),
        [52]: new Script(internalConstructor, internalConstructor, 52),
        [106]: new Script(internalConstructor, internalConstructor, 106),
        [127]: new Script(internalConstructor, internalConstructor, 127),
        [211]: new Script(internalConstructor, internalConstructor, 211),
        [153]: new Script(internalConstructor, internalConstructor, 153),
        [35]: new Script(internalConstructor, internalConstructor, 35),
        [195]: new Script(internalConstructor, internalConstructor, 195),
        [154]: new Script(internalConstructor, internalConstructor, 154),
        [36]: new Script(internalConstructor, internalConstructor, 36),
        [37]: new Script(internalConstructor, internalConstructor, 37),
        [38]: new Script(internalConstructor, internalConstructor, 38),
        [39]: new Script(internalConstructor, internalConstructor, 39),
        [60]: new Script(internalConstructor, internalConstructor, 60),
        [158]: new Script(internalConstructor, internalConstructor, 158),
        [212]: new Script(internalConstructor, internalConstructor, 212),
        [196]: new Script(internalConstructor, internalConstructor, 196),
        [53]: new Script(internalConstructor, internalConstructor, 53),
        [103]: new Script(internalConstructor, internalConstructor, 103),
        [99]: new Script(internalConstructor, internalConstructor, 99),
        [197]: new Script(internalConstructor, internalConstructor, 197),
        [188]: new Script(internalConstructor, internalConstructor, 188),
        [146]: new Script(internalConstructor, internalConstructor, 146),
        [192]: new Script(internalConstructor, internalConstructor, 192),
        [41]: new Script(internalConstructor, internalConstructor, 41),
        [177]: new Script(internalConstructor, internalConstructor, 177),
    };

    static Adlam = Script.#objectValues[167];
    static Ahom = Script.#objectValues[161];
    static AnatolianHieroglyphs = Script.#objectValues[156];
    static Arabic = Script.#objectValues[2];
    static Armenian = Script.#objectValues[3];
    static Avestan = Script.#objectValues[117];
    static Balinese = Script.#objectValues[62];
    static Bamum = Script.#objectValues[130];
    static BassaVah = Script.#objectValues[134];
    static Batak = Script.#objectValues[63];
    static Bengali = Script.#objectValues[4];
    static BeriaErfe = Script.#objectValues[208];
    static Bhaiksuki = Script.#objectValues[168];
    static Bopomofo = Script.#objectValues[5];
    static Brahmi = Script.#objectValues[65];
    static Braille = Script.#objectValues[46];
    static Buginese = Script.#objectValues[55];
    static Buhid = Script.#objectValues[44];
    static CanadianAboriginal = Script.#objectValues[40];
    static Carian = Script.#objectValues[104];
    static CaucasianAlbanian = Script.#objectValues[159];
    static Chakma = Script.#objectValues[118];
    static Cham = Script.#objectValues[66];
    static Cherokee = Script.#objectValues[6];
    static Chisoi = Script.#objectValues[209];
    static Chorasmian = Script.#objectValues[189];
    static Common = Script.#objectValues[0];
    static Coptic = Script.#objectValues[7];
    static Cuneiform = Script.#objectValues[101];
    static Cypriot = Script.#objectValues[47];
    static CyproMinoan = Script.#objectValues[193];
    static Cyrillic = Script.#objectValues[8];
    static Deseret = Script.#objectValues[9];
    static Devanagari = Script.#objectValues[10];
    static DivesAkuru = Script.#objectValues[190];
    static Dogra = Script.#objectValues[178];
    static Duployan = Script.#objectValues[135];
    static EgyptianHieroglyphs = Script.#objectValues[71];
    static Elbasan = Script.#objectValues[136];
    static Elymaic = Script.#objectValues[185];
    static Ethiopian = Script.#objectValues[11];
    static Georgian = Script.#objectValues[12];
    static Glagolitic = Script.#objectValues[56];
    static Gothic = Script.#objectValues[13];
    static Grantha = Script.#objectValues[137];
    static Greek = Script.#objectValues[14];
    static Gujarati = Script.#objectValues[15];
    static GunjalaGondi = Script.#objectValues[179];
    static Gurmukhi = Script.#objectValues[16];
    static Han = Script.#objectValues[17];
    static Hangul = Script.#objectValues[18];
    static HanifiRohingya = Script.#objectValues[182];
    static Hanunoo = Script.#objectValues[43];
    static Hatran = Script.#objectValues[162];
    static Hebrew = Script.#objectValues[19];
    static Hiragana = Script.#objectValues[20];
    static ImperialAramaic = Script.#objectValues[116];
    static Inherited = Script.#objectValues[1];
    static InscriptionalPahlavi = Script.#objectValues[122];
    static InscriptionalParthian = Script.#objectValues[125];
    static Javanese = Script.#objectValues[78];
    static Kaithi = Script.#objectValues[120];
    static Kannada = Script.#objectValues[21];
    static Katakana = Script.#objectValues[22];
    static Kawi = Script.#objectValues[198];
    static KayahLi = Script.#objectValues[79];
    static Kharoshthi = Script.#objectValues[57];
    static KhitanSmallScript = Script.#objectValues[191];
    static Khmer = Script.#objectValues[23];
    static Khojki = Script.#objectValues[157];
    static Khudawadi = Script.#objectValues[145];
    static Lao = Script.#objectValues[24];
    static Latin = Script.#objectValues[25];
    static Lepcha = Script.#objectValues[82];
    static Limbu = Script.#objectValues[48];
    static LinearA = Script.#objectValues[83];
    static LinearB = Script.#objectValues[49];
    static Lisu = Script.#objectValues[131];
    static Lycian = Script.#objectValues[107];
    static Lydian = Script.#objectValues[108];
    static Mahajani = Script.#objectValues[160];
    static Makasar = Script.#objectValues[180];
    static Malayalam = Script.#objectValues[26];
    static Mandaic = Script.#objectValues[84];
    static Manichaean = Script.#objectValues[121];
    static Marchen = Script.#objectValues[169];
    static MasaramGondi = Script.#objectValues[175];
    static Medefaidrin = Script.#objectValues[181];
    static MeeteiMayek = Script.#objectValues[115];
    static MendeKikakui = Script.#objectValues[140];
    static MeroiticCursive = Script.#objectValues[141];
    static MeroiticHieroglyphs = Script.#objectValues[86];
    static Miao = Script.#objectValues[92];
    static Modi = Script.#objectValues[163];
    static Mongolian = Script.#objectValues[27];
    static Mro = Script.#objectValues[149];
    static Multani = Script.#objectValues[164];
    static Myanmar = Script.#objectValues[28];
    static Nabataean = Script.#objectValues[143];
    static NagMundari = Script.#objectValues[199];
    static Nandinagari = Script.#objectValues[187];
    static Nastaliq = Script.#objectValues[200];
    static NewTaiLue = Script.#objectValues[59];
    static Newa = Script.#objectValues[170];
    static Nko = Script.#objectValues[87];
    static Nushu = Script.#objectValues[150];
    static NyiakengPuachueHmong = Script.#objectValues[186];
    static Ogham = Script.#objectValues[29];
    static OlChiki = Script.#objectValues[109];
    static OldHungarian = Script.#objectValues[76];
    static OldItalic = Script.#objectValues[30];
    static OldNorthArabian = Script.#objectValues[142];
    static OldPermic = Script.#objectValues[89];
    static OldPersian = Script.#objectValues[61];
    static OldSogdian = Script.#objectValues[184];
    static OldSouthArabian = Script.#objectValues[133];
    static OldTurkic = Script.#objectValues[88];
    static OldUyghur = Script.#objectValues[194];
    static Oriya = Script.#objectValues[31];
    static Osage = Script.#objectValues[171];
    static Osmanya = Script.#objectValues[50];
    static PahawhHmong = Script.#objectValues[75];
    static Palmyrene = Script.#objectValues[144];
    static PauCinHau = Script.#objectValues[165];
    static PhagsPa = Script.#objectValues[90];
    static Phoenician = Script.#objectValues[91];
    static PsalterPahlavi = Script.#objectValues[123];
    static Rejang = Script.#objectValues[110];
    static Runic = Script.#objectValues[32];
    static Samaritan = Script.#objectValues[126];
    static Saurashtra = Script.#objectValues[111];
    static Sharada = Script.#objectValues[151];
    static Shavian = Script.#objectValues[51];
    static Siddham = Script.#objectValues[166];
    static Sidetic = Script.#objectValues[210];
    static SignWriting = Script.#objectValues[112];
    static Sinhala = Script.#objectValues[33];
    static Sogdian = Script.#objectValues[183];
    static SoraSompeng = Script.#objectValues[152];
    static Soyombo = Script.#objectValues[176];
    static Sundanese = Script.#objectValues[113];
    static SylotiNagri = Script.#objectValues[58];
    static Syriac = Script.#objectValues[34];
    static Tagalog = Script.#objectValues[42];
    static Tagbanwa = Script.#objectValues[45];
    static TaiLe = Script.#objectValues[52];
    static TaiTham = Script.#objectValues[106];
    static TaiViet = Script.#objectValues[127];
    static TaiYo = Script.#objectValues[211];
    static Takri = Script.#objectValues[153];
    static Tamil = Script.#objectValues[35];
    static Tangsa = Script.#objectValues[195];
    static Tangut = Script.#objectValues[154];
    static Telugu = Script.#objectValues[36];
    static Thaana = Script.#objectValues[37];
    static Thai = Script.#objectValues[38];
    static Tibetan = Script.#objectValues[39];
    static Tifinagh = Script.#objectValues[60];
    static Tirhuta = Script.#objectValues[158];
    static TolongSiki = Script.#objectValues[212];
    static Toto = Script.#objectValues[196];
    static Ugaritic = Script.#objectValues[53];
    static Unknown = Script.#objectValues[103];
    static Vai = Script.#objectValues[99];
    static Vithkuqi = Script.#objectValues[197];
    static Wancho = Script.#objectValues[188];
    static WarangCiti = Script.#objectValues[146];
    static Yezidi = Script.#objectValues[192];
    static Yi = Script.#objectValues[41];
    static ZanabazarSquare = Script.#objectValues[177];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_Script_for_char_mv1(ch);

        try {
            return new Script(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_Script_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_Script_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.Script.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_Script_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.Script.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_Script_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new Script(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `SentenceBreak`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.SentenceBreak.html) for more information.
 */
class SentenceBreak {
    #value = undefined;

    static #values = new Map([
        ["Other", 0],
        ["ATerm", 1],
        ["Close", 2],
        ["Format", 3],
        ["Lower", 4],
        ["Numeric", 5],
        ["OLetter", 6],
        ["Sep", 7],
        ["Sp", 8],
        ["STerm", 9],
        ["Upper", 10],
        ["Cr", 11],
        ["Extend", 12],
        ["Lf", 13],
        ["SContinue", 14]
    ]);

    static getAllEntries() {
        return SentenceBreak.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return SentenceBreak.#objectValues[arguments[1]];
        }

        if (value instanceof SentenceBreak) {
            return value;
        }

        let intVal = SentenceBreak.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return SentenceBreak.#objectValues[intVal];
        }

        throw TypeError(value + " is not a SentenceBreak and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new SentenceBreak(value);
    }

    get value(){
        return [...SentenceBreak.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new SentenceBreak(internalConstructor, internalConstructor, 0),
        new SentenceBreak(internalConstructor, internalConstructor, 1),
        new SentenceBreak(internalConstructor, internalConstructor, 2),
        new SentenceBreak(internalConstructor, internalConstructor, 3),
        new SentenceBreak(internalConstructor, internalConstructor, 4),
        new SentenceBreak(internalConstructor, internalConstructor, 5),
        new SentenceBreak(internalConstructor, internalConstructor, 6),
        new SentenceBreak(internalConstructor, internalConstructor, 7),
        new SentenceBreak(internalConstructor, internalConstructor, 8),
        new SentenceBreak(internalConstructor, internalConstructor, 9),
        new SentenceBreak(internalConstructor, internalConstructor, 10),
        new SentenceBreak(internalConstructor, internalConstructor, 11),
        new SentenceBreak(internalConstructor, internalConstructor, 12),
        new SentenceBreak(internalConstructor, internalConstructor, 13),
        new SentenceBreak(internalConstructor, internalConstructor, 14),
    ];

    static Other = SentenceBreak.#objectValues[0];
    static ATerm = SentenceBreak.#objectValues[1];
    static Close = SentenceBreak.#objectValues[2];
    static Format = SentenceBreak.#objectValues[3];
    static Lower = SentenceBreak.#objectValues[4];
    static Numeric = SentenceBreak.#objectValues[5];
    static OLetter = SentenceBreak.#objectValues[6];
    static Sep = SentenceBreak.#objectValues[7];
    static Sp = SentenceBreak.#objectValues[8];
    static STerm = SentenceBreak.#objectValues[9];
    static Upper = SentenceBreak.#objectValues[10];
    static Cr = SentenceBreak.#objectValues[11];
    static Extend = SentenceBreak.#objectValues[12];
    static Lf = SentenceBreak.#objectValues[13];
    static SContinue = SentenceBreak.#objectValues[14];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_SentenceBreak_for_char_mv1(ch);

        try {
            return new SentenceBreak(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_SentenceBreak_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_SentenceBreak_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.SentenceBreak.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_SentenceBreak_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.SentenceBreak.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_SentenceBreak_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new SentenceBreak(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `VerticalOrientation`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.VerticalOrientation.html) for more information.
 */
class VerticalOrientation {
    #value = undefined;

    static #values = new Map([
        ["Rotated", 0],
        ["TransformedRotated", 1],
        ["TransformedUpright", 2],
        ["Upright", 3]
    ]);

    static getAllEntries() {
        return VerticalOrientation.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return VerticalOrientation.#objectValues[arguments[1]];
        }

        if (value instanceof VerticalOrientation) {
            return value;
        }

        let intVal = VerticalOrientation.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return VerticalOrientation.#objectValues[intVal];
        }

        throw TypeError(value + " is not a VerticalOrientation and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new VerticalOrientation(value);
    }

    get value(){
        return [...VerticalOrientation.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new VerticalOrientation(internalConstructor, internalConstructor, 0),
        new VerticalOrientation(internalConstructor, internalConstructor, 1),
        new VerticalOrientation(internalConstructor, internalConstructor, 2),
        new VerticalOrientation(internalConstructor, internalConstructor, 3),
    ];

    static Rotated = VerticalOrientation.#objectValues[0];
    static TransformedRotated = VerticalOrientation.#objectValues[1];
    static TransformedUpright = VerticalOrientation.#objectValues[2];
    static Upright = VerticalOrientation.#objectValues[3];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_VerticalOrientation_for_char_mv1(ch);

        try {
            return new VerticalOrientation(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_VerticalOrientation_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_VerticalOrientation_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.VerticalOrientation.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_VerticalOrientation_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.VerticalOrientation.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_VerticalOrientation_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new VerticalOrientation(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

// generated by diplomat-tool



/**
 * See the [Rust documentation for `WordBreak`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.WordBreak.html) for more information.
 */
class WordBreak {
    #value = undefined;

    static #values = new Map([
        ["Other", 0],
        ["ALetter", 1],
        ["Format", 2],
        ["Katakana", 3],
        ["MidLetter", 4],
        ["MidNum", 5],
        ["Numeric", 6],
        ["ExtendNumLet", 7],
        ["Cr", 8],
        ["Extend", 9],
        ["Lf", 10],
        ["MidNumLet", 11],
        ["Newline", 12],
        ["RegionalIndicator", 13],
        ["HebrewLetter", 14],
        ["SingleQuote", 15],
        ["DoubleQuote", 16],
        ["EBase", 17],
        ["EBaseGaz", 18],
        ["EModifier", 19],
        ["GlueAfterZwj", 20],
        ["Zwj", 21],
        ["WSegSpace", 22]
    ]);

    static getAllEntries() {
        return WordBreak.#values.entries();
    }

    #internalConstructor(value) {
        if (arguments.length > 1 && arguments[0] === internalConstructor) {
            // We pass in two internalConstructor arguments to create *new*
            // instances of this type, otherwise the enums are treated as singletons.
            if (arguments[1] === internalConstructor ) {
                this.#value = arguments[2];
                return this;
            }
            return WordBreak.#objectValues[arguments[1]];
        }

        if (value instanceof WordBreak) {
            return value;
        }

        let intVal = WordBreak.#values.get(value);

        // Nullish check, checks for null or undefined
        if (intVal != null) {
            return WordBreak.#objectValues[intVal];
        }

        throw TypeError(value + " is not a WordBreak and does not correspond to any of its enumerator values.");
    }

    /** @internal */
    static fromValue(value) {
        return new WordBreak(value);
    }

    get value(){
        return [...WordBreak.#values.keys()][this.#value];
    }

    /** @internal */
    get ffiValue(){
        return this.#value;
    }
    static #objectValues = [
        new WordBreak(internalConstructor, internalConstructor, 0),
        new WordBreak(internalConstructor, internalConstructor, 1),
        new WordBreak(internalConstructor, internalConstructor, 2),
        new WordBreak(internalConstructor, internalConstructor, 3),
        new WordBreak(internalConstructor, internalConstructor, 4),
        new WordBreak(internalConstructor, internalConstructor, 5),
        new WordBreak(internalConstructor, internalConstructor, 6),
        new WordBreak(internalConstructor, internalConstructor, 7),
        new WordBreak(internalConstructor, internalConstructor, 8),
        new WordBreak(internalConstructor, internalConstructor, 9),
        new WordBreak(internalConstructor, internalConstructor, 10),
        new WordBreak(internalConstructor, internalConstructor, 11),
        new WordBreak(internalConstructor, internalConstructor, 12),
        new WordBreak(internalConstructor, internalConstructor, 13),
        new WordBreak(internalConstructor, internalConstructor, 14),
        new WordBreak(internalConstructor, internalConstructor, 15),
        new WordBreak(internalConstructor, internalConstructor, 16),
        new WordBreak(internalConstructor, internalConstructor, 17),
        new WordBreak(internalConstructor, internalConstructor, 18),
        new WordBreak(internalConstructor, internalConstructor, 19),
        new WordBreak(internalConstructor, internalConstructor, 20),
        new WordBreak(internalConstructor, internalConstructor, 21),
        new WordBreak(internalConstructor, internalConstructor, 22),
    ];

    static Other = WordBreak.#objectValues[0];
    static ALetter = WordBreak.#objectValues[1];
    static Format = WordBreak.#objectValues[2];
    static Katakana = WordBreak.#objectValues[3];
    static MidLetter = WordBreak.#objectValues[4];
    static MidNum = WordBreak.#objectValues[5];
    static Numeric = WordBreak.#objectValues[6];
    static ExtendNumLet = WordBreak.#objectValues[7];
    static Cr = WordBreak.#objectValues[8];
    static Extend = WordBreak.#objectValues[9];
    static Lf = WordBreak.#objectValues[10];
    static MidNumLet = WordBreak.#objectValues[11];
    static Newline = WordBreak.#objectValues[12];
    static RegionalIndicator = WordBreak.#objectValues[13];
    static HebrewLetter = WordBreak.#objectValues[14];
    static SingleQuote = WordBreak.#objectValues[15];
    static DoubleQuote = WordBreak.#objectValues[16];
    static EBase = WordBreak.#objectValues[17];
    static EBaseGaz = WordBreak.#objectValues[18];
    static EModifier = WordBreak.#objectValues[19];
    static GlueAfterZwj = WordBreak.#objectValues[20];
    static Zwj = WordBreak.#objectValues[21];
    static WSegSpace = WordBreak.#objectValues[22];


    /**
     * See the [Rust documentation for `for_char`](https://docs.rs/icu/2.1.1/icu/properties/props/trait.EnumeratedProperty.html#tymethod.for_char) for more information.
     */
    static forChar(ch) {

        const result = wasm$1.icu4x_WordBreak_for_char_mv1(ch);

        try {
            return new WordBreak(internalConstructor, result);
        }

        finally {
        }
    }

    /**
     * Get the "long" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesLongBorrowed.html#method.get) for more information.
     */
    longName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_WordBreak_long_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Get the "short" name of this property value (returns empty if property value is unknown)
     *
     * See the [Rust documentation for `get`](https://docs.rs/icu/2.1.1/icu/properties/struct.PropertyNamesShortBorrowed.html#method.get) for more information.
     */
    shortName() {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 9, 4, true);


        wasm$1.icu4x_WordBreak_short_name_mv1(diplomatReceive.buffer, this.ffiValue);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new DiplomatSliceStr(wasm$1, diplomatReceive.buffer,  "string8", []).getValue();
        }

        finally {
            diplomatReceive.free();
        }
    }

    /**
     * Convert to an integer value usable with ICU4C and CodePointMapData
     *
     * See the [Rust documentation for `to_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.WordBreak.html#method.to_icu4c_value) for more information.
     */
    toIntegerValue() {

        const result = wasm$1.icu4x_WordBreak_to_integer_value_mv1(this.ffiValue);

        try {
            return result;
        }

        finally {
        }
    }

    /**
     * Convert from an integer value from ICU4C or CodePointMapData
     *
     * See the [Rust documentation for `from_icu4c_value`](https://docs.rs/icu/2.1.1/icu/properties/props/struct.WordBreak.html#method.from_icu4c_value) for more information.
     */
    static fromIntegerValue(other) {
        const diplomatReceive = new DiplomatReceiveBuf(wasm$1, 5, 4, true);


        wasm$1.icu4x_WordBreak_from_integer_value_mv1(diplomatReceive.buffer, other);

        try {
            if (!diplomatReceive.resultFlag) {
                return null;
            }
            return new WordBreak(internalConstructor, enumDiscriminant(wasm$1, diplomatReceive.buffer));
        }

        finally {
            diplomatReceive.free();
        }
    }

    constructor(value) {
        return this.#internalConstructor(...arguments)
    }
}

/**
 * Timezone conversion demo using ICU4X WASM bindings.
 *
 * Demonstrates converting dates between different timezones
 * using the ICU4X npm package.
 *
 * NOTE: The VariantOffsetsCalculator API is deprecated in ICU4X. It returns
 * both standard and daylight offsets but does not indicate which is currently
 * in effect. This demo uses the standard offset only, so DST-observing
 * timezones will show standard time year-round. A proper implementation
 * would need a full timezone database query to determine DST transitions.
 */


// --- Singletons ---
const ianaParser = new IanaParser();
const offsetCalc = new VariantOffsetsCalculator();

/**
 * Look up the standard UTC offset for a given IANA timezone name at a
 * specific UTC date and time.
 */
function getOffsetForTimezone(ianaName, utcDate, utcTime) {
  const tz = ianaParser.parse(ianaName);
  const offsets = offsetCalc.computeOffsetsFromTimeZoneAndDateTime(
    tz,
    utcDate,
    utcTime,
  );
  if (!offsets) {
    throw new Error(`Cannot compute offset for timezone: ${ianaName}`);
  }
  return offsets.standard;
}

/**
 * Format a UtcOffset as a string like "+05:30" or "-08:00".
 */
function formatOffset(offset) {
  const secs = offset.seconds;
  const sign = secs >= 0 ? '+' : '-';
  const abs = Math.abs(secs);
  const hrs = String(Math.floor(abs / 3600)).padStart(2, '0');
  const mins = String(Math.floor((abs % 3600) / 60)).padStart(2, '0');
  return `${sign}${hrs}:${mins}`;
}

/**
 * Convert epoch milliseconds to a local date/time string in a given timezone.
 */
function epochToLocal(epochMs, ianaName) {
  const tz = ianaParser.parse(ianaName);
  const epochMsBigInt = BigInt(epochMs);
  const offsets = offsetCalc.computeOffsetsFromTimeZoneAndTimestamp(
    tz,
    epochMsBigInt,
  );
  if (!offsets) {
    throw new Error(`Cannot compute offset for timezone: ${ianaName}`);
  }
  const offset = offsets.standard;

  const zdt = ZonedIsoDateTime.fromEpochMillisecondsAndUtcOffset(
    epochMsBigInt,
    offset,
  );

  const d = zdt.date;
  const t = zdt.time;
  const pad2 = (n) => String(n).padStart(2, '0');

  return `${d.year}-${pad2(d.month)}-${pad2(d.dayOfMonth)} ` +
    `${pad2(t.hour)}:${pad2(t.minute)}:${pad2(t.second)} ` +
    `(UTC${formatOffset(offset)})`;
}

/**
 * Convert a local date/time in one timezone to another timezone.
 * Takes an IXDTF string like "2025-01-15T10:30:00-05:00[America/New_York]".
 */
function convertTimezone(ixdtfString, targetIanaName) {
  const sourceZdt = ZonedIsoDateTime.strictFromString(ixdtfString, ianaParser);

  const srcDate = sourceZdt.date;
  const srcTime = sourceZdt.time;
  const srcOffset = sourceZdt.zone.offset;
  if (!srcOffset) {
    throw new Error('Source timezone has no offset');
  }

  // epoch = local_seconds - offset_seconds
  const localMs = Date.UTC(
    srcDate.year,
    srcDate.month - 1,
    srcDate.dayOfMonth,
    srcTime.hour,
    srcTime.minute,
    srcTime.second,
  );
  const epochMs = localMs - srcOffset.seconds * 1000;

  return epochToLocal(epochMs, targetIanaName);
}

/**
 * Show the current time in multiple timezones.
 */
function showWorldClocks(epochMs, timezones) {
  console.log(`\nWorld clocks (epoch: ${epochMs}):`);
  console.log('-'.repeat(60));
  for (const tz of timezones) {
    const local = epochToLocal(epochMs, tz);
    console.log(`  ${tz.padEnd(30)} ${local}`);
  }
}

// ============================================================
// Run the demo
// ============================================================

console.log('=== ICU4X Timezone Conversion Demo ===\n');

// 1. Convert a specific date/time from New York to several timezones
console.log('1. Converting 2025-07-04 10:30 AM New York (EDT, -04:00):\n');
const source = '2025-07-04T10:30:00-04:00[America/New_York]';
const targets = [
  'America/Los_Angeles',
  'Europe/London',
  'Europe/Berlin',
  'Asia/Tokyo',
  'Asia/Shanghai',
  'Australia/Sydney',
];

for (const target of targets) {
  const result = convertTimezone(source, target);
  console.log(`  -> ${target.padEnd(25)} ${result}`);
}

// 2. Show world clocks for a winter date
const winterEpoch = Date.UTC(2025, 0, 15, 12, 0, 0); // Jan 15, 2025 12:00 UTC
showWorldClocks(winterEpoch, [
  'UTC',
  'America/New_York',
  'America/Chicago',
  'America/Denver',
  'America/Los_Angeles',
  'Europe/London',
  'Europe/Paris',
  'Asia/Kolkata',
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Australia/Sydney',
  'Pacific/Auckland',
]);

// 3. Show world clocks for a summer date
const summerEpoch = Date.UTC(2025, 6, 15, 12, 0, 0); // Jul 15, 2025 12:00 UTC
showWorldClocks(summerEpoch, [
  'UTC',
  'America/New_York',
  'America/Chicago',
  'America/Denver',
  'America/Los_Angeles',
  'Europe/London',
  'Europe/Paris',
  'Asia/Kolkata',
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Australia/Sydney',
  'Pacific/Auckland',
]);

// 4. Demonstrate offset lookup for a timezone at a specific UTC time
console.log('\n4. UTC offset lookup (March 15, 2025 12:00 UTC):\n');
const utcDate = new IsoDate(2025, 3, 15);
const utcTime = new Time(12, 0, 0, 0);
const checkZones = [
  'America/New_York',
  'America/Los_Angeles',
  'Europe/London',
  'Asia/Tokyo',
];
for (const tz of checkZones) {
  const offset = getOffsetForTimezone(tz, utcDate, utcTime);
  console.log(
    `  ${tz.padEnd(25)} UTC${formatOffset(offset)} (${offset.seconds}s)`,
  );
}

console.log('\nDone.');
