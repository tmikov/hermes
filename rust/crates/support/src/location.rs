//! Offset-based source locations (Rust analog of `llvh::SMLoc`).

use std::num::NonZeroU32;

/// Opaque identifier of a source registered with the manager. 1-based: index 0
/// maps to `NonZeroU32(1)`, so `Option<SourceId>` is the same size as `SourceId`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SourceId(NonZeroU32);

impl SourceId {
    pub fn from_index(index: u32) -> SourceId {
        SourceId(NonZeroU32::new(index + 1).expect("index + 1 is nonzero"))
    }
    pub fn index(self) -> u32 {
        self.0.get() - 1
    }
}

/// A location in a source buffer. The "encoded" form: a buffer plus a byte
/// offset into it. Rust analog of `llvh::SMLoc`, but it carries its buffer
/// identity, so finding the containing buffer is trivial.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SMLoc {
    pub source: SourceId,
    pub offset: u32,
}

/// A half-open range of source locations `[start, end)` within one buffer.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SMRange {
    pub start: SMLoc,
    pub end: SMLoc,
}

impl SMRange {
    /// Build the smallest range covering both `a` and `b` (both must be in the
    /// same buffer). The end is exclusive, so it is `max(a,b).offset + 1`.
    /// Port of `SourceErrorManager::combineIntoRange`.
    pub fn combine(a: SMLoc, b: SMLoc) -> SMRange {
        debug_assert_eq!(a.source, b.source);
        let (lo, hi) = if a.offset <= b.offset { (a, b) } else { (b, a) };
        SMRange {
            start: lo,
            end: SMLoc {
                source: hi.source,
                offset: hi.offset + 1,
            },
        }
    }
}

/// The "decoded" form of an `SMLoc`: buffer id, 1-based line and column.
/// Port of `SourceErrorManager::SourceCoords`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SourceCoords {
    /// 1-based buffer id (i.e. `SourceId::index() + 1`).
    pub buf: SourceId,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
}

impl SourceCoords {
    pub fn is_same_source_line_as(&self, o: &SourceCoords) -> bool {
        self.buf == o.buf && self.line == o.line
    }
    pub fn less(&self, o: &SourceCoords) -> bool {
        (self.buf.index(), self.line, self.col) < (o.buf.index(), o.line, o.col)
    }
}

/// Result of looking up a line: buffer, 1-based line number, and a reference to
/// the line itself (including EOL if present). Port of `LineCoord`.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub struct LineCoord<'a> {
    pub buf: SourceId,
    pub line: u32,
    pub line_ref: &'a [u8],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_id_niche() {
        // Option<SourceId> must be the same size as SourceId (niche optimization).
        assert_eq!(
            std::mem::size_of::<Option<SourceId>>(),
            std::mem::size_of::<SourceId>()
        );
        assert_eq!(std::mem::size_of::<SMLoc>(), 8);
    }

    #[test]
    fn coords_ordering_and_same_line() {
        let s = SourceId::from_index(0);
        let a = SourceCoords {
            buf: s,
            line: 2,
            col: 3,
        };
        let b = SourceCoords {
            buf: s,
            line: 2,
            col: 5,
        };
        assert!(a.less(&b));
        assert!(a.is_same_source_line_as(&b));
    }

    #[test]
    fn combine_into_range_orders_endpoints() {
        let s = SourceId::from_index(0);
        let lo = SMLoc {
            source: s,
            offset: 4,
        };
        let hi = SMLoc {
            source: s,
            offset: 9,
        };
        let r = SMRange::combine(hi, lo);
        assert_eq!(r.start.offset, 4);
        assert_eq!(r.end.offset, 10); // end is exclusive: max + 1
    }
}
