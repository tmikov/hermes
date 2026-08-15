/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The GC arena is the single sanctioned location for encapsulated unsafe in
// this crate (see spec §1).
#![allow(unsafe_code)]

//! Garbage-collected Storage structures for AST nodes.

use std::cell::Cell;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use hermes_atom_table::AtomBytes;
use hermes_atom_table::AtomTable;

use hermes_support::deque::Deque;
use crate::node::Node;
use crate::NodeId;
use crate::node_child::NodeList;
use crate::visitor::Visitor;
use hermes_support::HeapSize;

/// ID which indicates a `StorageEntry` is free.
const FREE_ENTRY: u32 = 0;

/// Recover a pointer to the struct which contains `field`, where `offset` is
/// the byte offset of that field inside the containing struct (the C
/// `container_of` idiom).
///
/// The arithmetic is deliberately in *bytes*: `field` is a typed pointer, so
/// the plain `offset`/`sub` methods would step by `size_of::<Field>()` and
/// land somewhere far outside the object whenever the field is not at offset
/// zero. Nothing about a `repr(Rust)` struct guarantees a particular field
/// order, so byte stride is the only correct stride here.
///
/// # Safety
///
/// `field` must point to the field of a live `Outer` whose byte offset is
/// `offset` (i.e. `offset` came from `core::mem::offset_of!(Outer, <field>)`).
#[inline]
unsafe fn container_of<Outer, Field>(field: *const Field, offset: usize) -> *const Outer {
    // SAFETY: by the contract above, `field` is `offset` bytes into a live
    // `Outer`, so stepping back `offset` *bytes* stays inside that same
    // allocation and yields its base address.
    unsafe { field.byte_sub(offset).cast::<Outer>() }
}

/// A single entry in the heap.
#[derive(Debug)]
struct StorageEntry<'ctx> {
    /// ID of the context to which this entry belongs.
    /// Top bit is used as a mark bit, and flips meaning every time a GC happens.
    /// If this field is `0`, then this entry is free.
    ctx_id_markbit: Cell<u32>,

    /// Refcount of how many [`NodeRc`] point to this node.
    /// Entry may only be freed if this number is `0` and no other entries reference this entry
    /// directly.
    count: Cell<u32>,

    /// Actual node stored in this entry.
    inner: Node<'ctx>,
}

impl<'ctx> StorageEntry<'ctx> {
    /// Recover the `StorageEntry` which contains `node`.
    ///
    /// # Safety
    ///
    /// `node` must be a node allocated in a `Context` (i.e. it must be the
    /// `inner` field of a live `StorageEntry`), which is true of every node
    /// reference handed out by the arena.
    unsafe fn from_node<'a>(node: &'a Node<'a>) -> &'a StorageEntry<'a> {
        let inner_offset = core::mem::offset_of!(StorageEntry, inner);
        // SAFETY: by the contract above `node` is the `inner` field of a live
        // `StorageEntry`, so `container_of` yields that entry, which outlives
        // `'a` (entries never move once pushed into the deque).
        unsafe { &*container_of::<StorageEntry<'a>, Node<'a>>(node, inner_offset) }
    }

    #[inline]
    fn set_markbit(&self, bit: bool) {
        let id = self.ctx_id_markbit.get();
        if bit {
            self.ctx_id_markbit.set(id | 1 << 31);
        } else {
            self.ctx_id_markbit.set(id & !(1 << 31));
        }
    }

    #[inline]
    fn markbit(&self) -> bool {
        (self.ctx_id_markbit.get() >> 31) != 0
    }

    fn is_free(&self) -> bool {
        self.ctx_id_markbit.get() == FREE_ENTRY
    }
}

/// A single entry in the NodeList storage.
/// These are also immutable from the user's perspective, like `Node`s,
/// but they are temporarily mutated here during construction only, in order to append elements.
#[derive(Debug)]
pub(crate) struct NodeListElement<'ctx> {
    /// ID of the context to which this entry belongs.
    /// Top bit is used as a mark bit, and flips meaning every time a GC happens.
    /// If this field is `0`, then this entry is free.
    ctx_id_markbit: Cell<u32>,

    /// Actual node stored in this entry.
    /// Must not be null, because empty lists are represented as null pointers in the [`NodeList`].
    pub inner: *const Node<'ctx>,

    /// Pointer to the next element in the NodeList.
    /// Stored in a `Cell` to allow for simple appends.
    pub next: Cell<*const NodeListElement<'ctx>>,
}

impl<'ctx> NodeListElement<'ctx> {
    #[inline]
    fn set_markbit(&self, bit: bool) {
        let id = self.ctx_id_markbit.get();
        if bit {
            self.ctx_id_markbit.set(id | 1 << 31);
        } else {
            self.ctx_id_markbit.set(id & !(1 << 31));
        }
    }

    #[inline]
    fn markbit(&self) -> bool {
        (self.ctx_id_markbit.get() >> 31) != 0
    }

    fn is_free(&self) -> bool {
        self.ctx_id_markbit.get() == FREE_ENTRY
    }
}

/// Dereference a `NodeListElement`, returning its node and the next pointer.
/// The single sanctioned list-deref (see node_child::NodeListIter).
pub(crate) fn list_elem_parts<'gc>(
    ptr: *const NodeListElement<'gc>,
) -> (&'gc Node<'gc>, *const NodeListElement<'gc>) {
    let elem = unsafe { &*ptr };
    debug_assert!(!elem.inner.is_null(), "NodeList node must not be null");
    (unsafe { &*elem.inner }, elem.next.get())
}

/// Structure pointed to by `Context` and `NodeRc` to facilitate panicking if there are
/// outstanding `NodeRc` when the `Context` is dropped.
#[derive(Debug)]
struct NodeRcCounter {
    /// ID of the context owning the counter.
    ctx_id: u32,

    /// Number of [`NodeRc`]s allocated in this `Context`.
    /// Must be `0` when `Context` is dropped.
    count: Cell<usize>,
}

/// The storage for AST nodes.
///
/// Can be used to allocate and free nodes.
/// Nodes allocated in one `Context` must not be referenced by another `Context`'s AST.
#[derive(Debug)]
pub struct Context<'ast> {
    /// Unique number used to identify this context.
    id: u32,

    /// List of all the nodes stored in this context.
    /// Each element is a "chunk" of nodes.
    /// None of the chunks are ever resized after allocation.
    nodes: UnsafeCell<Deque<StorageEntry<'ast>>>,

    /// Free list for AST nodes.
    free_nodes: UnsafeCell<Vec<NonNull<StorageEntry<'ast>>>>,

    /// Every `NodeListElement` allocated in this context.
    /// These store the links in the linked lists.
    list_elements: UnsafeCell<Deque<NodeListElement<'ast>>>,

    /// Free list for `NodeListElement`s.
    free_list_elements: UnsafeCell<Vec<NonNull<NodeListElement<'ast>>>>,

    /// `NodeRc` count stored in a `Box` to ensure that `NodeRc`s can also point to it
    /// and decrement the count on drop.
    /// Placed separately to guard against `Context` moving, though relying on that behavior is
    /// technically unsafe.
    noderc_count: Pin<Box<NodeRcCounter>>,

    /// All identifiers are kept here.
    pub atom_table: AtomTable,

    /// `true` if `1` indicates an entry is marked, `false` if `0` indicates an entry is marked.
    /// Flipped every time GC occurs.
    markbit_marked: bool,

    /// Whether strict mode has been forced.
    strict_mode: bool,

    /// Is 'eval()' is enabled. Port of `Context::enableEval_`
    /// (Context.h:227-228); getter/setter at Context.h:407-412. Read by
    /// `SemanticResolver::visit(CallExpressionNode *)` (SemanticResolver.cpp:
    /// 1134) to decide between the `DirectEval` warning + `registerLocalEval`
    /// and the `EvalDisabled` warning. Default `true`, matching the C++
    /// member initializer (hermesc only turns it off for
    /// `-enable-eval=false`).
    enable_eval: bool,

    /// Whether to parse Flow type syntax. Mirrors C++ `Context::getParseFlow()`.
    parse_flow: bool,

    /// Whether to parse the Flow ambiguous-expression grammar (type-args on
    /// call/new, `as`, typed arrows, type-casts). Mirrors C++
    /// `Context::getParseFlowAmbiguous()` (= `parseFlow_ == ParseFlowSetting::ALL`).
    parse_flow_ambiguous: bool,

    /// Whether to parse Flow `component`/`hook` syntax. Mirrors C++
    /// `Context::getParseFlowComponentSyntax()`.
    parse_flow_component_syntax: bool,

    /// Whether to parse Flow `record` declarations/expressions. Mirrors C++
    /// `Context::getParseFlowRecords()`.
    parse_flow_records: bool,

    /// Whether to parse Flow `match` expressions/statements. Mirrors C++
    /// `Context::getParseFlowMatch()`.
    parse_flow_match: bool,

    /// Whether to parse TypeScript type syntax. Mirrors C++
    /// `Context::getParseTS()`.
    parse_ts: bool,

    /// Whether to parse JSX syntax. Mirrors C++ `Context::getParseJSX()`.
    /// Defaults to off; the TS `<Type>expr` assertion grammar is only enabled
    /// when JSX is *disabled* (C++ JSParserImpl.cpp:4164).
    parse_jsx: bool,

    /// Whether to warn about undefined variables in strict mode functions.
    pub warn_undefined: bool,

    /// Even if lazily compiling, eagerly compile any functions under this size
    /// in bytes. Port of `Context::preemptiveFunctionCompilationThreshold_`
    /// (Context.h:236); getter/setter at Context.h:516-521. Default `0`
    /// (= no threshold, consistent with the C++ initializer).
    preemptive_function_compilation_threshold: u32,

    /// Monotonic counter for `NodeId` assignment. Starts at `1` (`0` is
    /// `NodeId::UNASSIGNED`); `alloc` stamps the current value onto every
    /// node it establishes, then advances it. Never reset, never reused.
    next_node_id: Cell<u32>,

    /// Ids of nodes freed since the last `take_freed_node_ids()`. Appended to
    /// by both node-freeing paths: `gc()`'s sweep and `AllocationScope::drop`.
    /// Consumers (sema side tables) drain this to prune dead entries keyed
    /// by `NodeId` (see doc/superpowers/specs/2026-07-26-sema-untyped-design.md §3.1).
    freed_node_ids: RefCell<Vec<NodeId>>,
}

impl Default for Context<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> Context<'ast> {
    /// Allocate a new `Context` with a new ID.
    pub fn new() -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(FREE_ENTRY + 1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            nodes: Default::default(),
            free_nodes: Default::default(),
            list_elements: Default::default(),
            free_list_elements: Default::default(),
            noderc_count: Pin::new(Box::new(NodeRcCounter {
                ctx_id: id,
                count: Cell::new(0),
            })),
            atom_table: Default::default(),
            markbit_marked: true,
            strict_mode: false,
            enable_eval: true,
            parse_flow: false,
            parse_flow_ambiguous: false,
            parse_flow_component_syntax: false,
            parse_flow_records: false,
            parse_flow_match: false,
            parse_ts: false,
            parse_jsx: false,
            warn_undefined: false,
            preemptive_function_compilation_threshold: 0,
            next_node_id: Cell::new(1),
            freed_node_ids: RefCell::new(Vec::new()),
        }
    }

    /// Acquire a [`GCLock`] on this `Context`.
    /// This is just a more ergonomic way to call `GCLock::new`.
    pub fn lock<'ctx>(&'ctx mut self) -> GCLock<'ast, 'ctx> {
        GCLock::new(self)
    }

    /// Allocate a new `Node` in this `Context`.
    pub(crate) fn alloc<'s>(&'s self, n: Node<'_>) -> &'s Node<'s> {
        let free = unsafe { &mut *self.free_nodes.get() };
        let nodes: &mut Deque<StorageEntry<'ast>> = unsafe { &mut *self.nodes.get() };
        let node = unsafe { std::mem::transmute::<Node<'_>, Node<'_>>(n) };
        let entry: &StorageEntry<'ast> = if let Some(mut entry) = free.pop() {
            let entry: &mut StorageEntry<'ast> = unsafe { entry.as_mut() };
            debug_assert!(
                entry.ctx_id_markbit.get() == FREE_ENTRY,
                "Incorrect context ID"
            );
            debug_assert!(entry.count.get() == 0, "Freed entry has pointers to it");
            entry.ctx_id_markbit.set(self.id);
            entry.set_markbit(!self.markbit_marked);
            entry.inner = node;
            entry
        } else {
            let entry: &StorageEntry = nodes.push(StorageEntry {
                ctx_id_markbit: Cell::new(self.id),
                count: Cell::new(0),
                inner: node,
            });
            entry.set_markbit(!self.markbit_marked);
            entry
        };
        // Stamp a fresh, never-reused id unconditionally — both the
        // free-list-reuse and fresh-push arms land here.
        let id = self.next_node_id.get();
        self.next_node_id.set(id.checked_add(1).expect("NodeId overflow"));
        entry.inner.metadata().id.set(NodeId(id));
        // Transmute here to handle the fact that Cell<> is invariant over its type,
        // meaning the lifetime doesn't automatically narrow from `'ast` to `'s`.
        unsafe { std::mem::transmute(&entry.inner) }
    }

    /// Allocate a list element in the context with the provided previous element if it exists.
    /// `prev` will be updated to point to `node` as its next element.
    pub(crate) fn append_list_element<'a>(
        &'a self,
        prev: Option<&'a NodeListElement<'a>>,
        node: &'a Node<'a>,
    ) -> &'a NodeListElement<'a> {
        let elements: &mut Deque<NodeListElement<'ast>> = unsafe { &mut *self.list_elements.get() };
        let free = unsafe { &mut *self.free_list_elements.get() };
        // Transmutation is safe here, because `Node`s can only be allocated through
        // this path and only one GCLock can be made available at a time per thread.
        let node: &'ast Node<'ast> = unsafe { std::mem::transmute(node) };
        let prev: Option<&'ast NodeListElement<'ast>> = unsafe { std::mem::transmute(prev) };
        let entry = if let Some(mut entry) = free.pop() {
            let entry: &mut NodeListElement<'ast> = unsafe { entry.as_mut() };
            debug_assert!(
                entry.ctx_id_markbit.get() == FREE_ENTRY,
                "Incorrect context ID"
            );
            entry.ctx_id_markbit.set(self.id);
            entry.set_markbit(!self.markbit_marked);
            entry.inner = node;
            entry.next.set(std::ptr::null());
            if let Some(prev) = prev {
                prev.next.set(entry as *const _);
            }
            entry
        } else {
            let entry = elements.push(NodeListElement {
                ctx_id_markbit: Cell::new(self.id),
                inner: node,
                next: Cell::new(std::ptr::null()),
            });
            entry.set_markbit(!self.markbit_marked);
            if let Some(prev) = prev {
                prev.next.set(entry as *const _);
            }
            entry
        };
        debug_assert!(!entry.is_free(), "Entry must not be free");
        // Transmute here to handle the fact that Cell<> is invariant over its type,
        // meaning the lifetime doesn't automatically narrow from `'ast` to `'s`.
        unsafe { std::mem::transmute(entry) }
    }

    /// Return the atom table.
    pub fn atom_table(&self) -> &AtomTable {
        &self.atom_table
    }

    /// Add a byte-string to the identifier table.
    #[inline]
    pub fn atom_bytes<V: Into<Vec<u8>> + AsRef<[u8]>>(&self, value: V) -> AtomBytes {
        self.atom_table.atom_bytes(value)
    }

    /// Obtain the contents of an atom from the atom table.
    #[inline]
    pub fn bytes(&self, ident: AtomBytes) -> &[u8] {
        self.atom_table.bytes(ident)
    }

    /// Obtain the contents of an atom as a string, substituting U+FFFD for
    /// anything unrepresentable. See [`AtomTable::bytes_str_lossy`].
    #[inline]
    pub fn bytes_str_lossy(&self, ident: AtomBytes) -> &str {
        self.atom_table.bytes_str_lossy(ident)
    }

    /// Obtain the contents of an atom as a string, or `None` if they are not
    /// valid UTF-8. See [`AtomTable::try_bytes_str`].
    #[inline]
    pub fn try_bytes_str(&self, ident: AtomBytes) -> Option<&str> {
        self.atom_table.try_bytes_str(ident)
    }

    /// Return true if strict mode has been forced globally.
    pub fn strict_mode(&self) -> bool {
        self.strict_mode
    }

    /// Enable strict mode. Note that it cannot be unset.
    pub fn enable_strict_mode(&mut self) {
        self.strict_mode = true;
    }

    /// Return true if `eval()` is enabled. Mirrors C++
    /// `Context::getEnableEval()` (Context.h:407-409).
    pub fn enable_eval(&self) -> bool {
        self.enable_eval
    }

    /// Enable or disable `eval()`. Mirrors C++
    /// `Context::setEnableEval()` (Context.h:410-412).
    pub fn set_enable_eval(&mut self, v: bool) {
        self.enable_eval = v;
    }

    /// Return true if Flow type parsing is enabled.
    /// Mirrors C++ `Context::getParseFlow()`.
    pub fn parse_flow(&self) -> bool {
        self.parse_flow
    }

    /// Enable or disable Flow type parsing.
    /// Mirrors C++ `Context::setParseFlow()`.
    pub fn set_parse_flow(&mut self, v: bool) {
        self.parse_flow = v;
    }

    /// Return true if the Flow ambiguous-expression grammar is enabled.
    /// Mirrors C++ `Context::getParseFlowAmbiguous()`.
    pub fn parse_flow_ambiguous(&self) -> bool {
        self.parse_flow_ambiguous
    }

    /// Enable or disable the Flow ambiguous-expression grammar.
    pub fn set_parse_flow_ambiguous(&mut self, v: bool) {
        self.parse_flow_ambiguous = v;
    }

    /// Return true if Flow `component`/`hook` syntax is enabled.
    /// Mirrors C++ `Context::getParseFlowComponentSyntax()`.
    pub fn parse_flow_component_syntax(&self) -> bool {
        self.parse_flow_component_syntax
    }

    /// Enable or disable Flow `component`/`hook` syntax.
    pub fn set_parse_flow_component_syntax(&mut self, v: bool) {
        self.parse_flow_component_syntax = v;
    }

    /// Return true if Flow `record` declarations/expressions are enabled.
    /// Mirrors C++ `Context::getParseFlowRecords()`.
    pub fn parse_flow_records(&self) -> bool {
        self.parse_flow_records
    }

    /// Enable or disable Flow `record` declarations/expressions.
    pub fn set_parse_flow_records(&mut self, v: bool) {
        self.parse_flow_records = v;
    }

    /// Return true if Flow `match` expressions/statements are enabled.
    /// Mirrors C++ `Context::getParseFlowMatch()`.
    pub fn parse_flow_match(&self) -> bool {
        self.parse_flow_match
    }

    /// Enable or disable Flow `match` expressions/statements.
    pub fn set_parse_flow_match(&mut self, v: bool) {
        self.parse_flow_match = v;
    }

    /// Return true if TypeScript type parsing is enabled.
    /// Mirrors C++ `Context::getParseTS()`.
    pub fn parse_ts(&self) -> bool {
        self.parse_ts
    }

    /// Enable or disable TypeScript type parsing.
    /// Mirrors C++ `Context::setParseTS()`.
    pub fn set_parse_ts(&mut self, v: bool) {
        self.parse_ts = v;
    }

    /// Return true if JSX parsing is enabled. Mirrors C++
    /// `Context::getParseJSX()`.
    pub fn parse_jsx(&self) -> bool {
        self.parse_jsx
    }

    /// Enable or disable JSX parsing. Mirrors C++ `Context::setParseJSX()`.
    /// Currently only read by the TS `<Type>` cast gate; the setter is wired
    /// when the JSX phase lands.
    pub fn set_parse_jsx(&mut self, v: bool) {
        self.parse_jsx = v;
    }

    /// Return the preemptive-function-compilation threshold (bytes). Port of
    /// `Context::getPreemptiveFunctionCompilationThreshold()` (Context.h:516-518).
    pub fn preemptive_function_compilation_threshold(&self) -> u32 {
        self.preemptive_function_compilation_threshold
    }

    /// Set the preemptive-function-compilation threshold (bytes). Port of
    /// `Context::setPreemptiveFunctionCompilationThreshold()` (Context.h:520-522).
    pub fn set_preemptive_function_compilation_threshold(&mut self, byte_count: u32) {
        self.preemptive_function_compilation_threshold = byte_count;
    }

    /// Mark and sweep the arena: everything reachable from a live [`NodeRc`]
    /// survives, the rest is returned to the free lists. Requires `&mut self`,
    /// so no [`GCLock`] — and therefore no `&Node` — can be outstanding.
    pub fn gc(&mut self) {
        let nodes = unsafe { &mut *self.nodes.get() };
        let free_nodes = unsafe { &mut *self.free_nodes.get() };

        let list_elements = unsafe { &mut *self.list_elements.get() };
        let free_list_elements = unsafe { &mut *self.free_list_elements.get() };

        {
            // Begin by collecting all the roots: entries with non-zero refcount.
            let mut roots: Vec<&StorageEntry> = vec![];
            for entry in nodes.iter() {
                if entry.is_free() {
                    continue;
                }
                debug_assert!(
                    entry.markbit() != self.markbit_marked,
                    "Entry marked before start of GC: \
                        {:?}\nentry.markbit()={}\nmarkbit_marked={}",
                    &entry,
                    entry.markbit(),
                    self.markbit_marked,
                );
                if entry.count.get() > 0 {
                    // Transmuting the lifetime here because we have to store the roots from
                    // across accesses to `nodes`, meaning we must translate
                    // from `'ast` to the lifetime of this scope.
                    roots.push(unsafe {
                        std::mem::transmute::<&StorageEntry<'_>, &StorageEntry<'_>>(entry)
                    });
                }
            }

            struct Marker {
                markbit_marked: bool,
            }

            impl<'gc> Visitor<'gc> for Marker {
                fn visit_node(&mut self, node: &'gc Node<'gc>) {
                    let entry = unsafe { StorageEntry::from_node(node) };
                    if entry.markbit() == self.markbit_marked {
                        // Stop visiting early if we've already marked this part,
                        // because we must have also marked all the children.
                        return;
                    }
                    entry.set_markbit(self.markbit_marked);
                    let mark = self.markbit_marked;
                    node.mark_lists(&mut |list: &NodeList<'gc>| {
                        // Mark each list element's storage bit.
                        let mut p = list.head;
                        while !p.is_null() {
                            let elem = unsafe { &*p };
                            elem.set_markbit(mark);
                            p = elem.next.get();
                        }
                    });
                    node.visit_children(self);
                }
            }

            // Use a visitor to mark every node reachable from roots.
            // Marking happens while holding `&mut self`, so no GCLock
            // re-entrancy is needed.
            let mut marker = Marker {
                markbit_marked: self.markbit_marked,
            };
            for root in roots {
                marker.visit_node(&root.inner);
            }
        }

        // Borrow once: every node this sweep frees appends its id here so
        // sema side tables (keyed by NodeId) can prune the dead entries.
        let mut freed_node_ids = self.freed_node_ids.borrow_mut();
        for entry in nodes.iter_mut() {
            if entry.is_free() {
                // Skip free entries.
                continue;
            }
            if entry.count.get() > 0 {
                // Keep referenced entries alive.
                continue;
            }
            if entry.markbit() == self.markbit_marked {
                // Keep marked entries alive.
                continue;
            }
            // Passed all checks, this entry is free.
            freed_node_ids.push(entry.inner.metadata().id.get());
            entry.ctx_id_markbit.set(FREE_ENTRY);
            free_nodes.push(unsafe { NonNull::new_unchecked(entry as *mut StorageEntry) });
        }

        for element in list_elements.iter_mut() {
            if element.is_free() {
                // Skip free entries.
                continue;
            }
            if element.markbit() == self.markbit_marked {
                // Keep marked entries alive.
                continue;
            }
            // Passed all checks, this element is free.
            element.ctx_id_markbit.set(FREE_ENTRY);
            free_list_elements
                .push(unsafe { NonNull::new_unchecked(element as *mut NodeListElement) });
        }

        self.markbit_marked = !self.markbit_marked;
    }

    /// Drain and return the ids of every node freed (by `gc()` or by an
    /// `AllocationScope` truncation) since the last call. Consumers use this
    /// to prune dead entries out of side tables keyed by `NodeId`.
    pub fn take_freed_node_ids(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut *self.freed_node_ids.borrow_mut())
    }

    /// Returns the number of node slots which have been allocated.
    /// Includes nodes currently in use as well as nodes in the free list.
    pub fn num_nodes(&self) -> usize {
        let nodes = unsafe { &*self.nodes.get() };
        nodes.len()
    }

    /// Returns the number of list-element slots which have been allocated.
    /// Includes elements currently in use as well as elements in the free
    /// list.
    pub fn num_list_elements(&self) -> usize {
        let list_elements = unsafe { &*self.list_elements.get() };
        list_elements.len()
    }

    /// Returns the number of node slots currently in the free list (i.e.
    /// allocated but unused, reclaimed by GC).
    pub fn num_free_nodes(&self) -> usize {
        let free_nodes = unsafe { &*self.free_nodes.get() };
        free_nodes.len()
    }

    /// Returns the approximate size of just the AST storages in bytes.
    /// Includes the allocated nodes, lists, as well as free lists for both.
    pub fn storage_size(&self) -> usize {
        let nodes = unsafe { &*self.nodes.get() };
        let free_nodes = unsafe { &*self.free_nodes.get() };
        let list_elements = unsafe { &*self.list_elements.get() };
        let free_list_elements = unsafe { &*self.free_list_elements.get() };
        let mut result = 0;
        result += nodes.heap_size();
        result += free_nodes.heap_size();
        result += list_elements.heap_size();
        result += free_list_elements.heap_size();
        result
    }

    /// Leak everything an outstanding [`NodeRc`] still touches after this
    /// `Context` is gone, and return the leaked node storage.
    ///
    /// Called only from the `Drop` guard's failure path. A `NodeRc` that
    /// outlives its `Context` is a caller bug which the guard reports by
    /// panicking, but the report is worthless if the handle's own `drop` —
    /// which runs during the unwind, or after a `catch_unwind` — writes into
    /// freed memory. A handle reaches exactly two places: the `count` cell in
    /// its `StorageEntry` (inside the node deque) and the `NodeRcCounter`
    /// box (also read by `NodeRc::node` for its context-id check). Both are
    /// leaked here, so those accesses stay valid for the life of the process.
    /// Nothing else in the arena is reachable from a `NodeRc` once the
    /// `Context` is gone, so the rest is freed normally.
    fn leak_noderc_targets<'s>(&'s mut self) -> &'s Deque<StorageEntry<'ast>> {
        // SAFETY: `drop` holds `&mut self`, so no `GCLock` and no other
        // borrow of the deque exists; the field is left holding an empty
        // deque for the drop glue to dispose of.
        let nodes = std::mem::take(unsafe { &mut *self.nodes.get() });
        // The `StorageEntry`s live in the deque's chunks, which this moves
        // (as a `Vec<Vec<_>>` header) but does not reallocate, so entry
        // addresses — the ones the outstanding handles hold — are unchanged.
        let leaked_nodes: &'s Deque<StorageEntry<'ast>> = Box::leak(Box::new(nodes));

        // Replace the counter with a fresh box and forget the old one, so the
        // address every outstanding handle holds stays allocated. `ctx_id` is
        // preserved in the leaked copy, which keeps `NodeRc::node`'s
        // "allocated in context N" assertion honest afterwards.
        let fresh = Pin::new(Box::new(NodeRcCounter {
            ctx_id: self.id,
            count: Cell::new(0),
        }));
        std::mem::forget(std::mem::replace(&mut self.noderc_count, fresh));

        leaked_nodes
    }
}

impl HeapSize for Context<'_> {
    /// Returns the heap size of the AST storages only.
    /// Atom-table memory is intentionally excluded: the `AtomTable` is
    /// externally owned and accounted for separately.
    fn heap_size(&self) -> usize {
        let nodes = unsafe { &*self.nodes.get() };
        let free_nodes = unsafe { &*self.free_nodes.get() };
        let list_elements = unsafe { &*self.list_elements.get() };
        let free_list_elements = unsafe { &*self.free_list_elements.get() };
        let mut result = 0;
        result += nodes.heap_size();
        result += free_nodes.heap_size();
        result += list_elements.heap_size();
        result += free_list_elements.heap_size();
        result += std::mem::size_of::<NodeRcCounter>();
        result
    }
}

impl Drop for Context<'_> {
    /// Ensure that there are no outstanding `NodeRc`s into this `Context` which will be
    /// invalidated once it is dropped.
    ///
    /// # Panics
    ///
    /// Will panic if there are any `NodeRc`s stored when this `Context` is dropped.
    ///
    /// The panic is the *only* effect: before panicking, the node storage and
    /// the `NodeRc` counter are leaked (`Context::leak_noderc_targets`), so
    /// the outstanding handles — which are dropped during the ensuing
    /// unwind, or later still if the panic is caught — decrement refcounts in
    /// memory that is still valid. Leaking the arena is the price of keeping a
    /// caller's bug a panic instead of a use-after-free.
    fn drop(&mut self) {
        if self.noderc_count.count.get() > 0 {
            // Do this first: everything below can panic, and after the leak
            // no unwind path can free what the outstanding `NodeRc`s touch.
            let leaked_nodes = self.leak_noderc_targets();
            #[cfg(debug_assertions)]
            {
                // In debug mode, provide more information on which node was leaked.
                for entry in leaked_nodes.iter() {
                    assert!(
                        entry.count.get() == 0,
                        "NodeRc must not outlive Context: {:#?}\n",
                        &entry.inner
                    );
                }
            }
            #[cfg(not(debug_assertions))]
            let _ = leaked_nodes;
            // In release mode, just panic immediately.
            panic!("NodeRc must not outlive Context");
        }
    }
}

thread_local! {
    /// Whether there exists a `GCLock` on the current thread.
    static GCLOCK_IN_USE: Cell<bool> = const { Cell::new(false) };
}

/// A way to view the [`Context`].
///
/// Provides the user the ability to create new nodes and dereference [`NodeRc`].
///
/// **At most one is allowed to be active in any thread at any time.**
/// This is to ensure no `&Node` can be shared between `Context`s.
pub struct GCLock<'ast, 'ctx> {
    ctx: &'ctx mut Context<'ast>,
}

impl Drop for GCLock<'_, '_> {
    fn drop(&mut self) {
        GCLOCK_IN_USE.with(|flag| {
            flag.set(false);
        });
    }
}

impl<'ast, 'ctx> GCLock<'ast, 'ctx> {
    /// # Panics
    ///
    /// Will panic if there is already an active `GCLock` on this thread.
    pub fn new(ctx: &'ctx mut Context<'ast>) -> Self {
        GCLOCK_IN_USE.with(|flag| {
            if flag.get() {
                panic!("Attempt to create multiple GCLocks in a single thread");
            }
            flag.set(true);
        });
        GCLock { ctx }
    }

    /// Allocate a node in the `ctx`.
    #[inline]
    pub fn alloc<'s>(&'s self, n: Node<'s>) -> &'s Node<'s> {
        self.ctx.alloc(n)
    }

    /// Append `node` to the `prev` element if provided, else create the element as the first
    /// element in the `NodeList`.
    #[inline]
    pub(crate) fn append_list_element<'s>(
        &'s self,
        prev: Option<&'s NodeListElement<'s>>,
        n: &'s Node<'s>,
    ) -> &'s NodeListElement<'s> {
        self.ctx.append_list_element(prev, n)
    }

    /// Return a reference to the owning Context.
    pub fn ctx(&self) -> &Context<'ast> {
        self.ctx
    }

    /// Add a byte-string to the identifier table.
    #[inline]
    pub fn atom_bytes<V: Into<Vec<u8>> + AsRef<[u8]>>(&self, value: V) -> AtomBytes {
        self.ctx.atom_bytes(value)
    }

    /// Obtain the contents of an atom from the atom table.
    #[inline]
    pub fn bytes(&self, ident: AtomBytes) -> &[u8] {
        self.ctx.bytes(ident)
    }

    /// Obtain the contents of an atom as a string, substituting U+FFFD for
    /// anything unrepresentable. This is the usual way to print an
    /// identifier's name: `gc.bytes_str_lossy(id.name.get())`. See
    /// [`AtomTable::bytes_str_lossy`].
    #[inline]
    pub fn bytes_str_lossy(&self, ident: AtomBytes) -> &str {
        self.ctx.bytes_str_lossy(ident)
    }

    /// Obtain the contents of an atom as a string, or `None` if they are not
    /// valid UTF-8 — the right accessor for string-literal values, where
    /// substitution would silently corrupt the program's data. See
    /// [`AtomTable::try_bytes_str`].
    #[inline]
    pub fn try_bytes_str(&self, ident: AtomBytes) -> Option<&str> {
        self.ctx.try_bytes_str(ident)
    }
}

/// RAII allocation scope over the arena: everything allocated (nodes AND
/// list elements) between construction and drop is reclaimed at drop, with
/// bump-allocator save/restore semantics. Port of the C++ `AllocationScope`
/// (hermes/Support/Allocator.h:500-521) as used by the parser's PreParse
/// pass (JSParserImpl.cpp:548, 7523).
///
/// See [`GCLock::alloc_scope`] for the safety contract.
pub struct AllocationScope<'gcl, 'ast, 'ctx> {
    lock: &'gcl GCLock<'ast, 'ctx>,
    nodes_watermark: usize,
    list_elements_watermark: usize,
}

impl Drop for AllocationScope<'_, '_, '_> {
    fn drop(&mut self) {
        let ctx: &Context<'_> = self.lock.ctx;
        let nodes = unsafe { &mut *ctx.nodes.get() };
        #[cfg(debug_assertions)]
        for entry in nodes.iter_from(self.nodes_watermark) {
            // A NodeRc into the suffix would dangle after truncation.
            debug_assert!(
                entry.count.get() == 0,
                "NodeRc points into a truncated AllocationScope suffix"
            );
            // gc() cannot run under a GCLock, so no suffix entry can be
            // free (free-list pops reuse only pre-watermark slots).
            debug_assert!(!entry.is_free(), "free entry in scope suffix");
        }
        // Log every reclaimed node's id (the debug asserts above already
        // guarantee no suffix entry is free) so sema side tables can prune
        // dead entries, same as the gc() sweep does.
        let mut freed_node_ids = ctx.freed_node_ids.borrow_mut();
        for entry in nodes.iter_from(self.nodes_watermark) {
            freed_node_ids.push(entry.inner.metadata().id.get());
        }
        nodes.truncate(self.nodes_watermark);
        let list_elements = unsafe { &mut *ctx.list_elements.get() };
        list_elements.truncate(self.list_elements_watermark);
    }
}

impl<'ast, 'ctx> GCLock<'ast, 'ctx> {
    /// Open an allocation scope: everything allocated between this call and
    /// the returned guard's drop is reclaimed at drop (nodes and list
    /// elements). Mirrors the C++ `AllocationScope` discipline the PreParse
    /// pass uses (JSParserImpl.cpp:516-560).
    ///
    /// # Safety
    ///
    /// The caller must guarantee that when the guard drops:
    /// - no `&Node`, `NodeList`, `&NodeListElement`, or interior reference into an allocation
    ///   made after this call survives — the storage is freed and any such
    ///   reference dangles; and
    /// - no `NodeRc` points into those allocations (debug-asserted).
    ///
    /// If the `Context` ran `gc()` before this pass, in-scope allocations
    /// may be served from the free list at pre-watermark positions; those
    /// escape reclamation harmlessly (unreferenced until the next `gc()`).
    pub unsafe fn alloc_scope<'s>(&'s self) -> AllocationScope<'s, 'ast, 'ctx> {
        let nodes = unsafe { &*self.ctx.nodes.get() };
        let list_elements = unsafe { &*self.ctx.list_elements.get() };
        AllocationScope {
            lock: self,
            nodes_watermark: nodes.len(),
            list_elements_watermark: list_elements.len(),
        }
    }
}

/// A wrapper around Node&, with "shallow" hashing and equality, suitable for
/// hash tables.
#[derive(Debug, Copy, Clone)]
pub struct NodePtr<'gc>(pub &'gc Node<'gc>);

impl<'gc> NodePtr<'gc> {
    /// Wrap a node reference so it can be used as a hash-table key.
    pub fn from_node(node: &'gc Node<'gc>) -> Self {
        Self(node)
    }
}

impl<'gc> PartialEq for NodePtr<'gc> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for NodePtr<'_> {}

impl Hash for NodePtr<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0 as *const Node).hash(state)
    }
}

impl<'gc> Deref for NodePtr<'gc> {
    type Target = Node<'gc>;
    fn deref(&self) -> &'gc Self::Target {
        self.0
    }
}

impl<'gc> AsRef<Node<'gc>> for NodePtr<'gc> {
    fn as_ref(&self) -> &'gc Node<'gc> {
        self.0
    }
}

impl<'gc> From<&'gc Node<'gc>> for NodePtr<'gc> {
    fn from(node: &'gc Node<'gc>) -> Self {
        NodePtr(node)
    }
}

/// Reference counted pointer to a [`Node`] in any [`Context`].
///
/// It can be used to keep references to `Node`s outside of the lifetime of a [`GCLock`],
/// but the only way to derefence and inspect the `Node` is to use a `GCLock`.
///
/// A `NodeRc` must not outlive its `Context`: dropping a `Context` while one
/// is alive panics (see [`Context`]'s `Drop`). Should that happen anyway, the
/// handle itself stays safe to drop and to clone — the guard leaks the storage
/// it points at rather than freeing it — but it can no longer be dereferenced,
/// since [`NodeRc::node`] needs a `GCLock` on the context it came from.
#[derive(Debug, Eq)]
pub struct NodeRc {
    /// The `NodeRcCounter` counting for the `Context` to which this belongs.
    counter: NonNull<NodeRcCounter>,

    /// Pointer to the `StorageEntry` containing the `Node`.
    /// Stored as `c_void` to avoid specifying lifetimes, as dereferencing is checked manually.
    entry: NonNull<c_void>,
}

impl Hash for NodeRc {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entry.hash(state)
    }
}

impl PartialEq for NodeRc {
    fn eq(&self, other: &Self) -> bool {
        self.entry == other.entry
    }
}

impl Drop for NodeRc {
    fn drop(&mut self) {
        let entry = unsafe { self.entry().as_mut() };
        let c = entry.count.get();
        debug_assert!(c > 0);
        entry.count.set(c - 1);

        let noderc_count = unsafe { self.counter.as_mut() };
        let c = noderc_count.count.get();
        debug_assert!(c > 0);
        noderc_count.count.set(c - 1);
    }
}

impl Clone for NodeRc {
    /// Cloning a `NodeRc` increments refcounts on the entry and the context.
    fn clone(&self) -> Self {
        let mut cloned = NodeRc { ..*self };

        let entry = unsafe { cloned.entry().as_mut() };
        let c = entry.count.get();
        entry.count.set(c + 1);

        let noderc_count = unsafe { cloned.counter.as_mut() };
        let c = noderc_count.count.get();
        noderc_count.count.set(c + 1);

        cloned
    }
}

impl NodeRc {
    /// Turn a node reference into a `NodeRc` for storage outside `GCLock`.
    pub fn from_node<'gc>(gc: &'gc GCLock, node: &'gc Node<'gc>) -> NodeRc {
        // SAFETY: `node` was handed out by the arena, so it is the `inner`
        // field of a live `StorageEntry` — the contract of
        // `StorageEntry::from_node`.
        unsafe { Self::from_entry(gc, StorageEntry::from_node(node)) }
    }

    /// Return the actual `Node` that `self` points to.
    ///
    /// # Panics
    ///
    /// Will panic if `gc` is not for the same context as this `NodeRc` was created in.
    pub fn node<'gc>(&'_ self, gc: &'gc GCLock<'_, '_>) -> &'gc Node<'_> {
        unsafe {
            assert_eq!(
                self.counter.as_ref().ctx_id,
                gc.ctx.id,
                "Attempt to derefence NodeRc allocated context {} in context {}",
                self.counter.as_ref().ctx_id,
                gc.ctx.id
            );
            &self.entry().as_ref().inner
        }
    }

    /// Get the pointer to the `StorageEntry`.
    unsafe fn entry(&self) -> NonNull<StorageEntry<'_>> {
        let outer = self.entry.as_ptr() as *mut StorageEntry;
        NonNull::new_unchecked(outer)
    }

    unsafe fn from_entry(gc: &GCLock, entry: &StorageEntry<'_>) -> NodeRc {
        let c = entry.count.get();
        entry.count.set(c + 1);

        let c = gc.ctx.noderc_count.count.get();
        gc.ctx.noderc_count.count.set(c + 1);

        NodeRc {
            counter: NonNull::new_unchecked(gc.ctx.noderc_count.as_ref().get_ref()
                as *const NodeRcCounter
                as *mut NodeRcCounter),
            entry: NonNull::new_unchecked(entry as *const StorageEntry as *mut c_void),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::*;
    use crate::node_child::NodeMetadata;
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::panic::AssertUnwindSafe;

    fn dummy_range() -> hermes_support::location::SMRange {
        let l = hermes_support::location::SMLoc {
            source: hermes_support::location::SourceId::from_index(0),
            offset: 0,
        };
        hermes_support::location::SMRange { start: l, end: l }
    }

    fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
        gc.alloc(Node::NumericLiteral(NumericLiteral {
            metadata: NodeMetadata::new(dummy_range()),
            value: Cell::new(v),
        }))
    }

    #[test]
    fn alloc_and_deep_match() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let l = num(&gc, 1.0);
        let r = num(&gc, 2.0);
        let op = gc.atom_bytes("+".as_bytes());
        let bin = gc.alloc(Node::BinaryExpression(BinaryExpression {
            metadata: NodeMetadata::new(dummy_range()),
            left: l,
            right: r,
            operator: Cell::new(op),
        }));
        // Deep, one-level match through &Node.
        if let Node::BinaryExpression(b) = bin {
            assert!(matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 1.0));
        } else {
            panic!()
        }
    }

    #[test]
    fn cell_mutation_in_place() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let n = num(&gc, 3.0);
        if let Node::NumericLiteral(x) = n {
            x.value.set(9.0);
        }
        assert!(matches!(n, Node::NumericLiteral(x) if x.value.get() == 9.0));
    }

    #[test]
    #[should_panic(expected = "multiple GCLocks")]
    fn single_gclock_per_thread() {
        let mut a = Context::new();
        let mut b = Context::new();
        let _g1 = GCLock::new(&mut a);
        let _g2 = GCLock::new(&mut b); // must panic
    }

    #[test]
    fn from_iter_roundtrip() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);

        // Empty list has zero elements.
        let empty = NodeList::empty();
        assert_eq!(empty.iter().count(), 0);

        // Build three nodes and collect into a NodeList.
        let a = num(&gc, 1.0);
        let b = num(&gc, 2.0);
        let c = num(&gc, 3.0);
        let list = NodeList::from_iter(&gc, [a, b, c]);
        assert_eq!(list.iter().count(), 3);

        // Verify values come back in the original order.
        let values: Vec<f64> = list
            .iter()
            .map(|n| {
                if let Node::NumericLiteral(nl) = n {
                    nl.value.get()
                } else {
                    panic!("expected NumericLiteral")
                }
            })
            .collect();
        assert_eq!(values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn noderc_roundtrip() {
        let mut ctx = Context::new();
        let rc = {
            // First GCLock scope: allocate a node and wrap it in a NodeRc.
            let gc = GCLock::new(&mut ctx);
            let n = num(&gc, 42.0);
            NodeRc::from_node(&gc, n)
            // `gc` drops here, releasing the GCLock.
        };

        // Re-acquire the lock and verify the node is still reachable.
        let gc2 = GCLock::new(&mut ctx);
        let node = rc.node(&gc2);
        assert!(matches!(node, Node::NumericLiteral(nl) if nl.value.get() == 42.0));
        // Drop rc while the lock is held so the Context doesn't panic on drop.
        drop(rc);
    }

    /// The `StorageEntry` recovered from a node reference must be exactly the
    /// entry the allocation produced — for the node itself and for the
    /// `NodeRc` built from it.
    #[test]
    fn storage_entry_recovery_matches_allocation() {
        let mut ctx = Context::new();
        let rc = {
            let gc = GCLock::new(&mut ctx);
            let n = num(&gc, 7.0);

            // The entry the arena actually allocated: the last one pushed.
            let nodes = unsafe { &*gc.ctx().nodes.get() };
            let allocated = nodes.iter().last().expect("one entry") as *const StorageEntry as usize;

            let entry = unsafe { StorageEntry::from_node(n) };
            assert_eq!(
                entry as *const StorageEntry as usize, allocated,
                "StorageEntry::from_node must recover the allocated entry"
            );
            assert!(
                std::ptr::eq(&entry.inner, n),
                "recovered entry holds the node"
            );
            assert_eq!(entry.ctx_id_markbit.get() & !(1 << 31), gc.ctx().id);

            let rc = NodeRc::from_node(&gc, n);
            assert_eq!(
                rc.entry.as_ptr() as usize,
                allocated,
                "NodeRc::from_node must point at the allocated entry"
            );
            assert_eq!(entry.count.get(), 1, "the NodeRc took the entry's refcount");
            rc
        };
        let gc2 = GCLock::new(&mut ctx);
        assert!(matches!(rc.node(&gc2), Node::NumericLiteral(n) if n.value.get() == 7.0));
        drop(rc);
    }

    /// `container_of` must step back in *bytes*. `StorageEntry` is
    /// `repr(Rust)` and happens to place `inner` at offset 0 today, which
    /// hides the difference; this stand-in forces a non-zero offset, which is
    /// exactly what a field reorder would produce.
    #[test]
    fn container_of_is_byte_stride() {
        #[repr(C)]
        struct Outer {
            ctx_id_markbit: Cell<u32>,
            count: Cell<u32>,
            inner: [u64; 4],
        }
        let outer = Outer {
            ctx_id_markbit: Cell::new(1),
            count: Cell::new(0),
            inner: [7; 4],
        };
        let offset = core::mem::offset_of!(Outer, inner);
        assert_ne!(offset, 0, "the stand-in must exercise a non-zero offset");
        let recovered = unsafe { container_of::<Outer, [u64; 4]>(&outer.inner, offset) };
        assert_eq!(
            recovered as usize, &outer as *const Outer as usize,
            "container_of must step back in bytes, not in units of the field type"
        );
    }

    /// Build a `NodeRc`, park it in `escaped`, then drop its `Context` out
    /// from under it — which panics on the way out, by design.
    fn orphan_noderc(escaped: &RefCell<Option<NodeRc>>) {
        let mut ctx = Context::new();
        {
            let gc = GCLock::new(&mut ctx);
            *escaped.borrow_mut() = Some(NodeRc::from_node(&gc, num(&gc, 5.0)));
        }
        drop(ctx); // panics: a NodeRc is still alive
    }

    /// The documented guard: dropping a `Context` with a live `NodeRc` panics.
    /// `escaped` is a local of *this* function, so the orphaned handle is
    /// dropped by the unwind the guard starts — which is the first place the
    /// old code went off the rails (SIGSEGV, since a deque chunk is large
    /// enough to be unmapped on free).
    #[test]
    #[should_panic(expected = "NodeRc must not outlive Context")]
    fn noderc_outliving_context_panics() {
        let escaped = RefCell::new(None);
        orphan_noderc(&escaped);
    }

    /// ...and the panic is survivable, which is what a panic-catching host
    /// (test harness, server) depends on: here the handle outlives the caught
    /// panic and is cloned and dropped afterwards, touching both the entry's
    /// refcount and the counter's. Pre-fix those were accesses to freed
    /// memory.
    #[test]
    fn noderc_outliving_context_is_survivable() {
        // Declared outside the closure, so the handle survives the unwind.
        let escaped: RefCell<Option<NodeRc>> = RefCell::new(None);
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            orphan_noderc(&escaped);
        }));
        assert!(result.is_err(), "the guard must still panic");

        let rc = escaped
            .borrow_mut()
            .take()
            .expect("handle outlived the panic");
        let cloned = rc.clone(); // touches the leaked entry + counter
        drop(cloned);
        drop(rc);

        // Churn the allocator the way a panic-catching host would: pre-fix
        // the refcount decrements above landed in freed memory.
        let mut v: Vec<Vec<u64>> = (0..512u64).map(|i| vec![i; 64]).collect();
        v.truncate(0);
        drop(v);
    }

    #[test]
    fn alloc_scope_truncates_nodes_and_lists() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let base_nodes = gc.ctx().num_nodes();
        let base_elems = gc.ctx().num_list_elements();

        // Pre-scope survivor.
        let survivor = num(&gc, 99.0);
        {
            let _scope = unsafe { gc.alloc_scope() };
            for _ in 0..100 {
                num(&gc, 0.0);
            }
            // A NodeList inside the scope allocates list elements.
            let a = num(&gc, 1.0);
            let _list = NodeList::from_iter(&gc, [a]);
            assert_eq!(gc.ctx().num_nodes(), base_nodes + 102);
            assert!(gc.ctx().num_list_elements() > base_elems);
        }
        // Scope drop reclaimed everything allocated inside it.
        assert_eq!(gc.ctx().num_nodes(), base_nodes + 1);
        assert_eq!(gc.ctx().num_list_elements(), base_elems);
        // The pre-scope survivor is untouched.
        assert!(matches!(survivor, Node::NumericLiteral(n) if n.value.get() == 99.0));
    }

    #[test]
    fn alloc_scope_nests() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let base = gc.ctx().num_nodes();
        {
            let _outer = unsafe { gc.alloc_scope() };
            num(&gc, 1.0); // 1 outer allocation
            {
                let _inner = unsafe { gc.alloc_scope() };
                for _ in 0..50 {
                    num(&gc, 0.0);
                }
            }
            assert_eq!(gc.ctx().num_nodes(), base + 1, "inner scope reclaimed");
            // Outer keeps allocating after the inner truncate (bump reuse).
            for _ in 0..10 {
                num(&gc, 0.0);
            }
            assert_eq!(gc.ctx().num_nodes(), base + 11);
        }
        assert_eq!(gc.ctx().num_nodes(), base);
    }
}
