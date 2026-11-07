use crate::error::{Error, ErrorCode, Result};
use alloc::vec::Vec;
use core::cmp;
use core::ops::Deref;
use core::str;

#[cfg(feature = "std")]
use crate::io;
#[cfg(feature = "std")]
use crate::iter::LineColIterator;

#[cfg(feature = "raw_value")]
use crate::raw::BorrowedRawDeserializer;
#[cfg(all(feature = "raw_value", feature = "std"))]
use crate::raw::OwnedRawDeserializer;
#[cfg(all(feature = "raw_value", feature = "std"))]
use alloc::string::String;
#[cfg(feature = "raw_value")]
use serde::de::Visitor;

/// Trait used by the deserializer for iterating over input. This is manually
/// "specialized" for iterating over `&[u8]`. Once feature(specialization) is
/// stable we can use actual specialization.
///
/// This trait is sealed and cannot be implemented for types outside of
/// `serde_json`.
pub trait Read<'de>: private::Sealed {
    #[doc(hidden)]
    fn next(&mut self) -> Result<Option<u8>>;
    #[doc(hidden)]
    fn peek(&mut self) -> Result<Option<u8>>;

    /// Only valid after a call to peek(). Discards the peeked byte.
    #[doc(hidden)]
    fn discard(&mut self);

    /// Position of the most recent call to next().
    ///
    /// The most recent call was probably next() and not peek(), but this method
    /// should try to return a sensible result if the most recent call was
    /// actually peek() because we don't always know.
    ///
    /// Only called in case of an error, so performance is not important.
    #[doc(hidden)]
    fn position(&self) -> Position;

    /// Position of the most recent call to peek().
    ///
    /// The most recent call was probably peek() and not next(), but this method
    /// should try to return a sensible result if the most recent call was
    /// actually next() because we don't always know.
    ///
    /// Only called in case of an error, so performance is not important.
    #[doc(hidden)]
    fn peek_position(&self) -> Position;

    /// Offset from the beginning of the input to the next byte that would be
    /// returned by next() or peek().
    #[doc(hidden)]
    fn byte_offset(&self) -> usize;

    /// Assumes the previous byte was a quotation mark. Parses a Scon
    /// string until the next quotation mark using the given scratch space if
    /// necessary. The scratch space is initially empty.
    #[doc(hidden)]
    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>>;

    /// Parses a string until the next Scon marker
    #[doc(hidden)]
    fn parse_bare_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>>;

    /// Assumes the previous byte was a single quote. Parses a JSON-escaped
    /// string until the next single quote using the given scratch space if
    /// necessary. The scratch space is initially empty.
    #[doc(hidden)]
    fn parse_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>>;

    /// Parses a string until the next colon or Scon marker using the given
    /// scratch space if necessary. The scratch space is initially empty.
    /// A colon is followed by a key.
    #[doc(hidden)]
    fn parse_bare_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>>;

    /// Assumes the previous byte was a quotation mark. Parses a JSON-escaped
    /// string until the next quotation mark using the given scratch space if
    /// necessary. The scratch space is initially empty.
    ///
    /// This function returns the raw bytes in the string with escape sequences
    /// expanded but without performing unicode validation.
    #[doc(hidden)]
    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, [u8]>>;

    /// Assumes the previous byte was a quotation mark. Parses a JSON-escaped
    /// string until the next quotation mark but discards the data.
    #[doc(hidden)]
    fn ignore_str(&mut self) -> Result<()>;

    /// Assumes the previous byte was a single quote. Parses a Scon-escaped
    /// string until the next single quote but discards the data.
    #[doc(hidden)]
    fn ignore_key(&mut self) -> Result<()>;

    /// Switch raw buffering mode on.
    ///
    /// This is used when deserializing `RawValue`.
    #[cfg(feature = "raw_value")]
    #[doc(hidden)]
    fn begin_raw_buffering(&mut self);

    /// Switch raw buffering mode off and provides the raw buffered data to the
    /// given visitor.
    #[cfg(feature = "raw_value")]
    #[doc(hidden)]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>;

    /// Whether StreamDeserializer::next needs to check the failed flag. True
    /// for IoRead, false for StrRead and SliceRead which can track failure by
    /// truncating their input slice to avoid the extra check on every next
    /// call.
    #[doc(hidden)]
    const should_early_return_if_failed: bool;

    /// Mark a persistent failure of StreamDeserializer, either by setting the
    /// flag or by truncating the input data.
    #[doc(hidden)]
    fn set_failed(&mut self, failed: &mut bool);
}

pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub enum Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    Borrowed(&'b T),
    Copied(&'c T),
}

impl<'b, 'c, T> Deref for Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Reference::Borrowed(b) => b,
            Reference::Copied(c) => c,
        }
    }
}

/// JSON input source that reads from a std::io input stream.
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub struct IoRead<R>
where
    R: io::Read,
{
    iter: LineColIterator<io::Bytes<R>>,
    /// Temporary storage of peeked byte.
    ch: Option<u8>,
    #[cfg(feature = "raw_value")]
    raw_buffer: Option<Vec<u8>>,
    #[cfg(feature = "raw_value")]
    raw_buffering_start_position: Position,
}

/// JSON input source that reads from a slice of bytes.
//
// This is more efficient than other iterators because peek() can be read-only
// and we can compute line/col position only if an error happens.
pub struct SliceRead<'a> {
    slice: &'a [u8],
    /// Index of the *next* byte that will be returned by next() or peek().
    index: usize,
    #[cfg(feature = "raw_value")]
    raw_buffering_start_index: usize,
}

/// JSON input source that reads from a UTF-8 string.
//
// Able to elide UTF-8 checks by assuming that the input is valid UTF-8.
pub struct StrRead<'a> {
    delegate: SliceRead<'a>,
    #[cfg(feature = "raw_value")]
    data: &'a str,
}

// Prevent users from implementing the Read trait.
mod private {
    pub trait Sealed {}
}

//////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "std")]
impl<R> IoRead<R>
where
    R: io::Read,
{
    /// Create a JSON input source to read from a std::io input stream.
    ///
    /// When reading from a source against which short reads are not efficient, such
    /// as a [`File`], you will want to apply your own buffering because serde_json
    /// will not buffer the input. See [`std::io::BufReader`].
    ///
    /// [`File`]: std::fs::File
    pub fn new(reader: R) -> Self {
        IoRead {
            iter: LineColIterator::new(reader.bytes()),
            ch: None,
            #[cfg(feature = "raw_value")]
            raw_buffer: None,
            #[cfg(feature = "raw_value")]
            raw_buffering_start_position: Position { line: 1, column: 0 },
        }
    }
}

#[cfg(feature = "std")]
impl<R> private::Sealed for IoRead<R> where R: io::Read {}

#[cfg(feature = "std")]
impl<R> IoRead<R>
where
    R: io::Read,
{
    fn parse_str_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        quote: u8,
        result: F,
    ) -> Result<T>
    where
        T: 's,
        F: FnOnce(&'s Self, &'s [u8]) -> Result<T>,
    {
        loop {
            let ch = tri!(next_or_eof(self));
            if ch != quote {
                scratch.push(ch);
                continue;
            }
            if ch == quote {
                match tri!(self.peek()) {
                    Some(q) if q == quote => _ = self.next(),
                    _ => return result(self, scratch),
                }
            }
            scratch.push(ch);
        }
    }

    fn parse_bare_str_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        result: F,
    ) -> Result<T>
    where
        T: 's,
        F: FnOnce(&'s Self, &'s [u8]) -> Result<T>,
    {
        loop {
            let ch = tri!(next_or_eof(self));
            match ch {
                b' ' | b']' | b'}' | b',' => {
                    return result(self, scratch);
                }
                _ => {
                    scratch.push(ch);
                }
            }
        }
    }

    fn parse_bare_key_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        result: F,
    ) -> Result<T>
    where
        T: 's,
        F: FnOnce(&'s Self, &'s [u8]) -> Result<T>,
    {
        loop {
            let ch = tri!(next_or_eof(self));
            match ch {
                b':' => {
                    scratch.clear();
                }
                b' ' | b'*' | b'~' | b'/' | b'?' | b'"' | b'^' | //"
                b'<' | b'[' | b']' | b'{' | b'}' | b',' => {
                    return result(self, scratch);
                }
                _ => {
                    scratch.push(ch);
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl<'de, R> Read<'de> for IoRead<R>
where
    R: io::Read,
{
    #[inline]
    fn next(&mut self) -> Result<Option<u8>> {
        match self.ch.take() {
            Some(ch) => {
                #[cfg(feature = "raw_value")]
                {
                    if let Some(buf) = &mut self.raw_buffer {
                        buf.push(ch);
                    }
                }
                Ok(Some(ch))
            }
            None => match self.iter.next() {
                Some(Err(err)) => Err(Error::io(err)),
                Some(Ok(ch)) => {
                    #[cfg(feature = "raw_value")]
                    {
                        if let Some(buf) = &mut self.raw_buffer {
                            buf.push(ch);
                        }
                    }
                    Ok(Some(ch))
                }
                None => Ok(None),
            },
        }
    }

    #[inline]
    fn peek(&mut self) -> Result<Option<u8>> {
        match self.ch {
            Some(ch) => Ok(Some(ch)),
            None => match self.iter.next() {
                Some(Err(err)) => Err(Error::io(err)),
                Some(Ok(ch)) => {
                    self.ch = Some(ch);
                    Ok(self.ch)
                }
                None => Ok(None),
            },
        }
    }

    #[cfg(not(feature = "raw_value"))]
    #[inline]
    fn discard(&mut self) {
        self.ch = None;
    }

    #[cfg(feature = "raw_value")]
    fn discard(&mut self) {
        if let Some(ch) = self.ch.take() {
            if let Some(buf) = &mut self.raw_buffer {
                buf.push(ch);
            }
        }
    }

    fn position(&self) -> Position {
        Position {
            line: self.iter.line(),
            column: self.iter.col(),
        }
    }

    fn peek_position(&self) -> Position {
        // The LineColIterator updates its position during peek() so it has the
        // right one here.
        self.position()
    }

    fn byte_offset(&self) -> usize {
        match self.ch {
            Some(_) => self.iter.byte_offset() - 1,
            None => self.iter.byte_offset(),
        }
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        self.parse_str_bytes(scratch, b'"', as_str) //"
            .map(Reference::Copied)
    }

    fn parse_bare_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        self.parse_bare_str_bytes(scratch, as_str)
            .map(Reference::Copied)
    }

    fn parse_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        self.parse_str_bytes(scratch, b'\'', as_str)
            .map(Reference::Copied)
    }

    fn parse_bare_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        self.parse_bare_key_bytes(scratch, as_str)
            .map(Reference::Copied)
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, [u8]>> {
        self.parse_str_bytes(scratch, b'"', |_, bytes| Ok(bytes)) //"
            .map(Reference::Copied)
    }

    fn ignore_str(&mut self) -> Result<()> {
        loop {
            let ch = tri!(next_or_eof(self));
            if ch == b'"' {
                match tri!(self.peek()) {
                    Some(b'"') => _ = self.next(),
                    _ => return Ok(()),
                }
            }
        }
    }

    fn ignore_key(&mut self) -> Result<()> {
        loop {
            let ch = tri!(next_or_eof(self));
            if ch == b'\'' {
                match tri!(self.peek()) {
                    Some(b'\'') => _ = self.next(),
                    _ => return Ok(()),
                }
            }
        }
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        let position = self.position();
        self.raw_buffering_start_position = Position {
            line: position.line,
            column: position.column - usize::from(self.ch.is_some()),
        };
        self.raw_buffer = Some(Vec::new());
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw_buffer.take().unwrap();
        let raw = match String::from_utf8(raw) {
            Ok(raw) => raw,
            Err(err) => {
                let prefix = &err.as_bytes()[..=err.utf8_error().valid_up_to()];
                let line;
                let column;
                if let Some(last_newline) = memchr::memrchr(b'\n', prefix) {
                    line = self.raw_buffering_start_position.line
                        + memchr::memchr_iter(b'\n', &prefix[..last_newline]).count()
                        + 1;
                    column = prefix.len() - last_newline - 1;
                } else {
                    line = self.raw_buffering_start_position.line;
                    column = self.raw_buffering_start_position.column + prefix.len();
                }
                return Err(Error::syntax(
                    ErrorCode::InvalidUnicodeCodePoint,
                    line,
                    column,
                ));
            }
        };
        visitor.visit_map(OwnedRawDeserializer {
            raw_value: Some(raw),
        })
    }

    const should_early_return_if_failed: bool = true;

    #[inline]
    #[cold]
    fn set_failed(&mut self, failed: &mut bool) {
        *failed = true;
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a> SliceRead<'a> {
    /// Create a JSON input source to read from a slice of bytes.
    pub fn new(slice: &'a [u8]) -> Self {
        SliceRead {
            slice,
            index: 0,
            #[cfg(feature = "raw_value")]
            raw_buffering_start_index: 0,
        }
    }

    fn position_of_index(&self, i: usize) -> Position {
        let start_of_line = match memchr::memrchr(b'\n', &self.slice[..i]) {
            Some(position) => position + 1,
            None => 0,
        };
        Position {
            line: 1 + memchr::memchr_iter(b'\n', &self.slice[..start_of_line]).count(),
            column: i - start_of_line,
        }
    }

    fn skip_to_quote(&mut self, quote: u8) {
        // Immediately bail-out on empty strings
        if self.index == self.slice.len() {
            return;
        }

        match memchr::memchr(quote, &self.slice[self.index..]) {
            Some(len) => self.index += len,
            None => self.index = self.slice.len(),
        }
    }

    /// The big optimization here over IoRead is that if the string contains no
    /// backslash escape sequences, the returned &str is a slice of the raw Scon
    /// data so we avoid copying into the scratch space.
    fn parse_str_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        quote: u8,
        result: F,
    ) -> Result<Reference<'a, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'f> FnOnce(&'s Self, &'f [u8]) -> Result<&'f T>,
    {
        // Index of the first byte not yet copied into the scratch space.
        let mut start = self.index;

        loop {
            self.skip_to_quote(quote);
            if self.index == self.slice.len() {
                return error(self, ErrorCode::EofWhileParsingString);
            }
            if self.index + 1 == self.slice.len() || self.slice[self.index + 1] != quote {
                if scratch.is_empty() {
                    // Fast path: return a slice of the raw Scon without any
                    // copying.
                    let borrowed = &self.slice[start..self.index];
                    self.index += 1;
                    return result(self, borrowed).map(Reference::Borrowed);
                } else {
                    scratch.extend_from_slice(&self.slice[start..self.index]);
                    self.index += 1;
                    return result(self, scratch).map(Reference::Copied);
                }
            } else {
                scratch.extend_from_slice(&self.slice[start..=self.index]);
                self.index += 2;
                start = self.index;
            }
        }
    }

    fn parse_bare_str_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        result: F,
    ) -> Result<Reference<'a, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'f> FnOnce(&'s Self, &'f [u8]) -> Result<&'f T>,
    {
        // Index of the first byte
        let start = self.index;

        while self.index < self.slice.len() {
            match self.slice[self.index] {
                0..=b' ' | b'}' | b']' | b',' => {
                    break;
                }
                _ => {
                    self.index += 1;
                }
            }
        }
        // Fast path: return a slice of the raw Scon without any
        // copying.
        let borrowed = &self.slice[start..self.index];
        return result(self, borrowed).map(Reference::Borrowed);
    }

    fn parse_bare_key_bytes<'s, T, F>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
        result: F,
    ) -> Result<Reference<'a, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'f> FnOnce(&'s Self, &'f [u8]) -> Result<&'f T>,
    {
        // Index of the first byte
        let start = self.index;

        while self.index < self.slice.len() {
            match self.slice[self.index] {
                0..=b' ' | b':' | b'*' | b'~' | b'/' | b'?' | b'"' | //"
                b'^' | b'<' | b'[' | b']' | b'{' | b'}' | b',' => {
                    break;
                }
                _ => {
                    self.index += 1;
                }
            }
        }
        // Fast path: return a slice of the raw Scon without any
        // copying.
        let borrowed = &self.slice[start..self.index];
        return result(self, borrowed).map(Reference::Borrowed);
    }
}

impl<'a> private::Sealed for SliceRead<'a> {}

impl<'a> Read<'a> for SliceRead<'a> {
    #[inline]
    fn next(&mut self) -> Result<Option<u8>> {
        // `Ok(self.slice.get(self.index).map(|ch| { self.index += 1; *ch }))`
        // is about 10% slower.
        Ok(if self.index < self.slice.len() {
            let ch = self.slice[self.index];
            self.index += 1;
            Some(ch)
        } else {
            None
        })
    }

    #[inline]
    fn peek(&mut self) -> Result<Option<u8>> {
        // `Ok(self.slice.get(self.index).map(|ch| *ch))` is about 10% slower
        // for some reason.
        Ok(if self.index < self.slice.len() {
            Some(self.slice[self.index])
        } else {
            None
        })
    }

    #[inline]
    fn discard(&mut self) {
        self.index += 1;
    }

    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    fn peek_position(&self) -> Position {
        // Cap it at slice.len() just in case the most recent call was next()
        // and it returned the last byte.
        self.position_of_index(cmp::min(self.slice.len(), self.index + 1))
    }

    fn byte_offset(&self) -> usize {
        self.index
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.parse_str_bytes(scratch, b'"', as_str) //"
    }

    fn parse_bare_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.parse_bare_str_bytes(scratch, as_str)
    }

    fn parse_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.parse_str_bytes(scratch, b'\'', as_str)
    }

    fn parse_bare_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.parse_bare_key_bytes(scratch, as_str)
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'a, 's, [u8]>> {
        self.parse_str_bytes(scratch, b'"', |_, bytes| Ok(bytes)) //"
    }
    
    fn ignore_str(&mut self) -> Result<()> {
        loop {
            self.skip_to_quote(b'"'); //"
            if self.index == self.slice.len() {
                return error(self, ErrorCode::EofWhileParsingString);
            }
            self.index += 1;
            if self.index == self.slice.len() || self.slice[self.index] != b'"' { // " not doubled
                return Ok(());
            }
            self.index += 1;
        }
    }

    fn ignore_key(&mut self) -> Result<()> {
        loop {
            self.skip_to_quote(b'\'');
            if self.index == self.slice.len() {
                return error(self, ErrorCode::EofWhileParsingString);
            }
            self.index += 1;
            if self.index == self.slice.len() || self.slice[self.index] != b'\'' { // ' not doubled
                return Ok(());
            }
            self.index += 1;
        }
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        self.raw_buffering_start_index = self.index;
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'a>,
    {
        let raw = &self.slice[self.raw_buffering_start_index..self.index];
        let raw = match str::from_utf8(raw) {
            Ok(raw) => raw,
            Err(err) => {
                let at = self.raw_buffering_start_index + err.valid_up_to() + 1;
                let position = self.position_of_index(at);
                return Err(Error::syntax(
                    ErrorCode::InvalidUnicodeCodePoint,
                    position.line,
                    position.column,
                ));
            }
        };
        visitor.visit_map(BorrowedRawDeserializer {
            raw_value: Some(raw),
        })
    }

    const should_early_return_if_failed: bool = false;

    #[inline]
    #[cold]
    fn set_failed(&mut self, _failed: &mut bool) {
        self.slice = &self.slice[..self.index];
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a> StrRead<'a> {
    /// Create a JSON input source to read from a UTF-8 string.
    pub fn new(s: &'a str) -> Self {
        StrRead {
            delegate: SliceRead::new(s.as_bytes()),
            #[cfg(feature = "raw_value")]
            data: s,
        }
    }
}

impl<'a> private::Sealed for StrRead<'a> {}

impl<'a> Read<'a> for StrRead<'a> {
    #[inline]
    fn next(&mut self) -> Result<Option<u8>> {
        self.delegate.next()
    }

    #[inline]
    fn peek(&mut self) -> Result<Option<u8>> {
        self.delegate.peek()
    }

    #[inline]
    fn discard(&mut self) {
        self.delegate.discard();
    }

    fn position(&self) -> Position {
        self.delegate.position()
    }

    fn peek_position(&self) -> Position {
        self.delegate.peek_position()
    }

    fn byte_offset(&self) -> usize {
        self.delegate.byte_offset()
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.delegate.parse_str_bytes(scratch, b'"', |_, bytes| { //"
            // The deserialization input came in as &str with a UTF-8 guarantee
            Ok(unsafe { str::from_utf8_unchecked(bytes) })
        })
    }

    fn parse_bare_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.delegate.parse_bare_str_bytes(scratch, |_, bytes| {
            // The deserialization input came in as &str with a UTF-8 guarantee
            Ok(unsafe { str::from_utf8_unchecked(bytes) })
        })
    }

    fn parse_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.delegate.parse_str_bytes(scratch, b'\'', |_, bytes| {
            // The deserialization input came in as &str with a UTF-8 guarantee,
            Ok(unsafe { str::from_utf8_unchecked(bytes) })
        })
    }

    fn parse_bare_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>> {
        self.delegate.parse_bare_key_bytes(scratch, |_, bytes| {
            // The deserialization input came in as &str with a UTF-8 guarantee,
            Ok(unsafe { str::from_utf8_unchecked(bytes) })
        })
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'a, 's, [u8]>> {
        self.delegate.parse_str_raw(scratch)
    }

    fn ignore_str(&mut self) -> Result<()> {
        self.delegate.ignore_str()
    }

    fn ignore_key(&mut self) -> Result<()> {
        self.delegate.ignore_key()
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        self.delegate.begin_raw_buffering();
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'a>,
    {
        let raw = &self.data[self.delegate.raw_buffering_start_index..self.delegate.index];
        visitor.visit_map(BorrowedRawDeserializer {
            raw_value: Some(raw),
        })
    }

    const should_early_return_if_failed: bool = false;

    #[inline]
    #[cold]
    fn set_failed(&mut self, failed: &mut bool) {
        self.delegate.set_failed(failed);
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'de, R> private::Sealed for &mut R where R: Read<'de> {}

impl<'de, R> Read<'de> for &mut R
where
    R: Read<'de>,
{
    fn next(&mut self) -> Result<Option<u8>> {
        R::next(self)
    }

    fn peek(&mut self) -> Result<Option<u8>> {
        R::peek(self)
    }

    fn discard(&mut self) {
        R::discard(self);
    }

    fn position(&self) -> Position {
        R::position(self)
    }

    fn peek_position(&self) -> Position {
        R::peek_position(self)
    }

    fn byte_offset(&self) -> usize {
        R::byte_offset(self)
    }

    fn parse_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        R::parse_str(self, scratch)
    }

    fn parse_bare_str<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        R::parse_bare_str(self, scratch)
    }

    fn parse_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        R::parse_key(self, scratch)
    }

    fn parse_bare_key<'s>(&'s mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>> {
        R::parse_bare_key(self, scratch)
    }

    fn parse_str_raw<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, [u8]>> {
        R::parse_str_raw(self, scratch)
    }

    fn ignore_str(&mut self) -> Result<()> {
        R::ignore_str(self)
    }

    fn ignore_key(&mut self) -> Result<()> {
        R::ignore_key(self)
    }

    #[cfg(feature = "raw_value")]
    fn begin_raw_buffering(&mut self) {
        R::begin_raw_buffering(self);
    }

    #[cfg(feature = "raw_value")]
    fn end_raw_buffering<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        R::end_raw_buffering(self, visitor)
    }

    const should_early_return_if_failed: bool = R::should_early_return_if_failed;

    fn set_failed(&mut self, failed: &mut bool) {
        R::set_failed(self, failed);
    }
}

//////////////////////////////////////////////////////////////////////////////

/// Marker for whether StreamDeserializer can implement FusedIterator.
pub trait Fused: private::Sealed {}
impl<'a> Fused for SliceRead<'a> {}
impl<'a> Fused for StrRead<'a> {}

fn next_or_eof<'de, R>(read: &mut R) -> Result<u8>
where
    R: ?Sized + Read<'de>,
{
    match tri!(read.next()) {
        Some(b) => Ok(b),
        None => error(read, ErrorCode::EofWhileParsingString),
    }
}

fn peek_or_eof<'de, R>(read: &mut R) -> Result<u8>
where
    R: ?Sized + Read<'de>,
{
    match tri!(read.peek()) {
        Some(b) => Ok(b),
        None => error(read, ErrorCode::EofWhileParsingString),
    }
}

fn error<'de, R, T>(read: &R, reason: ErrorCode) -> Result<T>
where
    R: ?Sized + Read<'de>,
{
    let position = read.position();
    Err(Error::syntax(reason, position.line, position.column))
}

fn as_str<'de, 's, R: Read<'de>>(read: &R, slice: &'s [u8]) -> Result<&'s str> {
    match str::from_utf8(slice) {
        Ok(s) => Ok(s),
        Err(err) => {
            // Point at the first invalid UTF-8 byte. `read` is positioned at
            // the closing quote of the string whose content is `slice`. If the
            // JSON string contained escape sequences, the column position is
            // approximate.
            let position = read.position();
            Err(Error::syntax(
                ErrorCode::InvalidUnicodeCodePoint,
                position.line,
                position
                    .column
                    .saturating_sub(slice.len() - err.valid_up_to()),
            ))
        }
    }
}
