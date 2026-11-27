//! Charts (FR-22 to FR-27, ADR 0022). The nodes of a graph are pure code in
//! `track_record_core::chart::eval`; this module does the I/O around them.
//! [`source`] loads the frames of a source node.

pub mod source;
