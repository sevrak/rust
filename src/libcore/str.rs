// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
 * String manipulation
 *
 * Strings are a packed UTF-8 representation of text, stored as null
 * terminated buffers of u8 bytes.  Strings should be indexed in bytes,
 * for efficiency, but UTF-8 unsafe operations should be avoided.  For
 * some heavy-duty uses, try std::rope.
 */

use at_vec;
use cast;
use char;
use clone::Clone;
use cmp::{Equiv, TotalOrd, Ordering, Less, Equal, Greater};
use libc;
use option::{None, Option, Some};
use ptr;
use str;
use u8;
use uint;
use vec;
use to_str::ToStr;

#[cfg(notest)] use cmp::{Eq, Ord};

/*
Section: Creating a string
*/

/**
 * Convert a vector of bytes to a UTF-8 string
 *
 * # Failure
 *
 * Fails if invalid UTF-8
 */
pub fn from_bytes(vv: &[const u8]) -> ~str {
    fail_unless!(is_utf8(vv));
    return unsafe { raw::from_bytes(vv) };
}

/// Copy a slice into a new unique str
pub fn from_slice(s: &str) -> ~str {
    unsafe { raw::slice_bytes_unique(s, 0, len(s)) }
}

impl ToStr for ~str {
    #[inline(always)]
    fn to_str(&self) -> ~str { copy *self }
}
impl ToStr for &'self str {
    #[inline(always)]
    fn to_str(&self) -> ~str { ::str::from_slice(*self) }
}
impl ToStr for @str {
    #[inline(always)]
    fn to_str(&self) -> ~str { ::str::from_slice(*self) }
}

/**
 * Convert a byte to a UTF-8 string
 *
 * # Failure
 *
 * Fails if invalid UTF-8
 */
pub fn from_byte(b: u8) -> ~str {
    fail_unless!(b < 128u8);
    unsafe { ::cast::transmute(~[b, 0u8]) }
}

/// Appends a character at the end of a string
pub fn push_char(s: &mut ~str, ch: char) {
    unsafe {
        let code = ch as uint;
        let nb = if code < max_one_b { 1u }
        else if code < max_two_b { 2u }
        else if code < max_three_b { 3u }
        else if code < max_four_b { 4u }
        else if code < max_five_b { 5u }
        else { 6u };
        let len = len(*s);
        let new_len = len + nb;
        reserve_at_least(&mut *s, new_len);
        let off = len;
        do as_buf(*s) |buf, _len| {
            let buf: *mut u8 = ::cast::reinterpret_cast(&buf);
            if nb == 1u {
                *ptr::mut_offset(buf, off) =
                    code as u8;
            } else if nb == 2u {
                *ptr::mut_offset(buf, off) =
                    (code >> 6u & 31u | tag_two_b) as u8;
                *ptr::mut_offset(buf, off + 1u) =
                    (code & 63u | tag_cont) as u8;
            } else if nb == 3u {
                *ptr::mut_offset(buf, off) =
                    (code >> 12u & 15u | tag_three_b) as u8;
                *ptr::mut_offset(buf, off + 1u) =
                    (code >> 6u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 2u) =
                    (code & 63u | tag_cont) as u8;
            } else if nb == 4u {
                *ptr::mut_offset(buf, off) =
                    (code >> 18u & 7u | tag_four_b) as u8;
                *ptr::mut_offset(buf, off + 1u) =
                    (code >> 12u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 2u) =
                    (code >> 6u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 3u) =
                    (code & 63u | tag_cont) as u8;
            } else if nb == 5u {
                *ptr::mut_offset(buf, off) =
                    (code >> 24u & 3u | tag_five_b) as u8;
                *ptr::mut_offset(buf, off + 1u) =
                    (code >> 18u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 2u) =
                    (code >> 12u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 3u) =
                    (code >> 6u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 4u) =
                    (code & 63u | tag_cont) as u8;
            } else if nb == 6u {
                *ptr::mut_offset(buf, off) =
                    (code >> 30u & 1u | tag_six_b) as u8;
                *ptr::mut_offset(buf, off + 1u) =
                    (code >> 24u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 2u) =
                    (code >> 18u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 3u) =
                    (code >> 12u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 4u) =
                    (code >> 6u & 63u | tag_cont) as u8;
                *ptr::mut_offset(buf, off + 5u) =
                    (code & 63u | tag_cont) as u8;
            }
        }

        raw::set_len(s, new_len);
    }
}

/// Convert a char to a string
pub fn from_char(ch: char) -> ~str {
    let mut buf = ~"";
    unsafe { push_char(&mut buf, ch); }
    buf
}

/// Convert a vector of chars to a string
pub fn from_chars(chs: &[char]) -> ~str {
    let mut buf = ~"";
    unsafe {
        reserve(&mut buf, chs.len());
        for vec::each(chs) |ch| {
            push_char(&mut buf, *ch);
        }
    }
    buf
}

/// Appends a string slice to the back of a string, without overallocating
#[inline(always)]
pub fn push_str_no_overallocate(lhs: &mut ~str, rhs: &str) {
    unsafe {
        let llen = lhs.len();
        let rlen = rhs.len();
        reserve(&mut *lhs, llen + rlen);
        do as_buf(*lhs) |lbuf, _llen| {
            do as_buf(rhs) |rbuf, _rlen| {
                let dst = ptr::offset(lbuf, llen);
                let dst = ::cast::transmute_mut_unsafe(dst);
                ptr::copy_memory(dst, rbuf, rlen);
            }
        }
        raw::set_len(lhs, llen + rlen);
    }
}
/// Appends a string slice to the back of a string
#[inline(always)]
pub fn push_str(lhs: &mut ~str, rhs: &str) {
    unsafe {
        let llen = lhs.len();
        let rlen = rhs.len();
        reserve_at_least(&mut *lhs, llen + rlen);
        do as_buf(*lhs) |lbuf, _llen| {
            do as_buf(rhs) |rbuf, _rlen| {
                let dst = ptr::offset(lbuf, llen);
                let dst = ::cast::transmute_mut_unsafe(dst);
                ptr::copy_memory(dst, rbuf, rlen);
            }
        }
        raw::set_len(lhs, llen + rlen);
    }
}

/// Concatenate two strings together
#[inline(always)]
pub fn append(lhs: ~str, rhs: &str) -> ~str {
    let mut v = lhs;
    unsafe {
        push_str_no_overallocate(&mut v, rhs);
    }
    v
}


/// Concatenate a vector of strings
pub fn concat(v: &[~str]) -> ~str {
    let mut s: ~str = ~"";
    for vec::each(v) |ss| {
        unsafe { push_str(&mut s, *ss) };
    }
    s
}

/// Concatenate a vector of strings, placing a given separator between each
pub fn connect(v: &[~str], sep: &str) -> ~str {
    let mut s = ~"", first = true;
    for vec::each(v) |ss| {
        if first { first = false; } else { unsafe { push_str(&mut s, sep); } }
        unsafe { push_str(&mut s, *ss) };
    }
    s
}

/// Concatenate a vector of strings, placing a given separator between each
pub fn connect_slices(v: &[&str], sep: &str) -> ~str {
    let mut s = ~"", first = true;
    for vec::each(v) |ss| {
        if first { first = false; } else { unsafe { push_str(&mut s, sep); } }
        unsafe { push_str(&mut s, *ss) };
    }
    s
}

/// Given a string, make a new string with repeated copies of it
pub fn repeat(ss: &str, nn: uint) -> ~str {
    let mut acc = ~"";
    for nn.times { acc += ss; }
    acc
}

/*
Section: Adding to and removing from a string
*/

/**
 * Remove the final character from a string and return it
 *
 * # Failure
 *
 * If the string does not contain any characters
 */
pub fn pop_char(s: &mut ~str) -> char {
    let end = len(*s);
    fail_unless!(end > 0u);
    let CharRange {ch, next} = char_range_at_reverse(*s, end);
    unsafe { raw::set_len(s, next); }
    return ch;
}

/**
 * Remove the first character from a string and return it
 *
 * # Failure
 *
 * If the string does not contain any characters
 */
pub fn shift_char(s: &mut ~str) -> char {
    let CharRange {ch, next} = char_range_at(*s, 0u);
    *s = unsafe { raw::slice_bytes_unique(*s, next, len(*s)) };
    return ch;
}

/**
 * Removes the first character from a string slice and returns it. This does
 * not allocate a new string; instead, it mutates a slice to point one
 * character beyond the character that was shifted.
 *
 * # Failure
 *
 * If the string does not contain any characters
 */
#[inline]
pub fn slice_shift_char(s: &'a str) -> (char, &'a str) {
    let CharRange {ch, next} = char_range_at(s, 0u);
    let next_s = unsafe { raw::slice_bytes(s, next, len(s)) };
    return (ch, next_s);
}

/// Prepend a char to a string
pub fn unshift_char(s: &mut ~str, ch: char) {
    *s = from_char(ch) + *s;
}

/**
 * Returns a string with leading `chars_to_trim` removed.
 *
 * # Arguments
 *
 * * s - A string
 * * chars_to_trim - A vector of chars
 *
 */
pub fn trim_left_chars(s: &'a str, chars_to_trim: &[char]) -> &'a str {
    if chars_to_trim.is_empty() { return s; }

    match find(s, |c| !chars_to_trim.contains(&c)) {
      None => "",
      Some(first) => unsafe { raw::slice_bytes(s, first, s.len()) }
    }
}

/**
 * Returns a string with trailing `chars_to_trim` removed.
 *
 * # Arguments
 *
 * * s - A string
 * * chars_to_trim - A vector of chars
 *
 */
pub fn trim_right_chars(s: &'a str, chars_to_trim: &[char]) -> &'a str {
    if chars_to_trim.is_empty() { return s; }

    match rfind(s, |c| !chars_to_trim.contains(&c)) {
      None => "",
      Some(last) => {
        let next = char_range_at(s, last).next;
        unsafe { raw::slice_bytes(s, 0u, next) }
      }
    }
}

/**
 * Returns a string with leading and trailing `chars_to_trim` removed.
 *
 * # Arguments
 *
 * * s - A string
 * * chars_to_trim - A vector of chars
 *
 */
pub fn trim_chars(s: &'a str, chars_to_trim: &[char]) -> &'a str {
    trim_left_chars(trim_right_chars(s, chars_to_trim), chars_to_trim)
}

/// Returns a string with leading whitespace removed
pub fn trim_left(s: &'a str) -> &'a str {
    match find(s, |c| !char::is_whitespace(c)) {
      None => "",
      Some(first) => unsafe { raw::slice_bytes(s, first, len(s)) }
    }
}

/// Returns a string with trailing whitespace removed
pub fn trim_right(s: &'a str) -> &'a str {
    match rfind(s, |c| !char::is_whitespace(c)) {
      None => "",
      Some(last) => {
        let next = char_range_at(s, last).next;
        unsafe { raw::slice_bytes(s, 0u, next) }
      }
    }
}

/// Returns a string with leading and trailing whitespace removed
pub fn trim(s: &'a str) -> &'a str { trim_left(trim_right(s)) }

/*
Section: Transforming strings
*/

/**
 * Converts a string to a vector of bytes
 *
 * The result vector is not null-terminated.
 */
pub fn to_bytes(s: &str) -> ~[u8] {
    unsafe {
        let mut v: ~[u8] = ::cast::transmute(from_slice(s));
        vec::raw::set_len(&mut v, len(s));
        v
    }
}

/// Work with the string as a byte slice, not including trailing null.
#[inline(always)]
pub fn byte_slice<T>(s: &str, f: &fn(v: &[u8]) -> T) -> T {
    do as_buf(s) |p,n| {
        unsafe { vec::raw::buf_as_slice(p, n-1u, f) }
    }
}

/// Convert a string to a vector of characters
pub fn chars(s: &str) -> ~[char] {
    let mut buf = ~[], i = 0;
    let len = len(s);
    while i < len {
        let CharRange {ch, next} = char_range_at(s, i);
        unsafe { buf.push(ch); }
        i = next;
    }
    buf
}

/**
 * Take a substring of another.
 *
 * Returns a string containing `n` characters starting at byte offset
 * `begin`.
 */
pub fn substr(s: &'a str, begin: uint, n: uint) -> &'a str {
    slice(s, begin, begin + count_bytes(s, begin, n))
}

/**
 * Returns a slice of the given string from the byte range [`begin`..`end`)
 *
 * Fails when `begin` and `end` do not point to valid characters or beyond
 * the last character of the string
 */
pub fn slice(s: &'a str, begin: uint, end: uint) -> &'a str {
    fail_unless!(is_char_boundary(s, begin));
    fail_unless!(is_char_boundary(s, end));
    unsafe { raw::slice_bytes(s, begin, end) }
}

/// Splits a string into substrings at each occurrence of a given
/// character.
pub fn split_char(s: &str, sep: char) -> ~[~str] {
    split_char_inner(s, sep, len(s), true, true)
}

/**
 * Splits a string into substrings at each occurrence of a given
 * character up to 'count' times.
 *
 * The byte must be a valid UTF-8/ASCII byte
 */
pub fn splitn_char(s: &str, sep: char, count: uint) -> ~[~str] {
    split_char_inner(s, sep, count, true, true)
}

/// Like `split_char`, but omits empty strings from the returned vector
pub fn split_char_nonempty(s: &str, sep: char) -> ~[~str] {
    split_char_inner(s, sep, len(s), false, false)
}

/**
 * Like `split_char`, but a trailing empty string is omitted
 * (e.g. `split_char_no_trailing("A B ",' ') == ~[~"A",~"B"]`)
 */
pub fn split_char_no_trailing(s: &str, sep: char) -> ~[~str] {
    split_char_inner(s, sep, len(s), true, false)
}

fn split_char_inner(s: &str, sep: char, count: uint, allow_empty: bool,
                    allow_trailing_empty: bool) -> ~[~str] {
    if sep < 128u as char {
        let b = sep as u8, l = len(s);
        let mut result = ~[], done = 0u;
        let mut i = 0u, start = 0u;
        while i < l && done < count {
            if s[i] == b {
                if allow_empty || start < i {
                    unsafe {
                        result.push(raw::slice_bytes_unique(s, start, i));
                    }
                }
                start = i + 1u;
                done += 1u;
            }
            i += 1u;
        }
        // only push a non-empty trailing substring
        if allow_trailing_empty || start < l {
            unsafe { result.push(raw::slice_bytes_unique(s, start, l) ) };
        }
        result
    } else {
        split_inner(s, |cur| cur == sep, count, allow_empty, allow_trailing_empty)
    }
}


/// Splits a string into substrings using a character function
pub fn split(s: &str, sepfn: &fn(char) -> bool) -> ~[~str] {
    split_inner(s, sepfn, len(s), true, true)
}

/**
 * Splits a string into substrings using a character function, cutting at
 * most `count` times.
 */
pub fn splitn(s: &str,
                   sepfn: &fn(char) -> bool,
                   count: uint)
                -> ~[~str] {
    split_inner(s, sepfn, count, true, true)
}

/// Like `split`, but omits empty strings from the returned vector
pub fn split_nonempty(s: &str, sepfn: &fn(char) -> bool) -> ~[~str] {
    split_inner(s, sepfn, len(s), false, false)
}


/**
 * Like `split`, but a trailing empty string is omitted
 * (e.g. `split_no_trailing("A B ",' ') == ~[~"A",~"B"]`)
 */
pub fn split_no_trailing(s: &str, sepfn: &fn(char) -> bool) -> ~[~str] {
    split_inner(s, sepfn, len(s), true, false)
}

fn split_inner(s: &str, sepfn: &fn(cc: char) -> bool, count: uint,
               allow_empty: bool, allow_trailing_empty: bool) -> ~[~str] {
    let l = len(s);
    let mut result = ~[], i = 0u, start = 0u, done = 0u;
    while i < l && done < count {
        let CharRange {ch, next} = char_range_at(s, i);
        if sepfn(ch) {
            if allow_empty || start < i {
                unsafe {
                    result.push(raw::slice_bytes_unique(s, start, i));
                }
            }
            start = next;
            done += 1u;
        }
        i = next;
    }
    if allow_trailing_empty || start < l {
        unsafe {
            result.push(raw::slice_bytes_unique(s, start, l));
        }
    }
    result
}

// See Issue #1932 for why this is a naive search
fn iter_matches(s: &'a str, sep: &'b str, f: &fn(uint, uint)) {
    let sep_len = len(sep), l = len(s);
    fail_unless!(sep_len > 0u);
    let mut i = 0u, match_start = 0u, match_i = 0u;

    while i < l {
        if s[i] == sep[match_i] {
            if match_i == 0u { match_start = i; }
            match_i += 1u;
            // Found a match
            if match_i == sep_len {
                f(match_start, i + 1u);
                match_i = 0u;
            }
            i += 1u;
        } else {
            // Failed match, backtrack
            if match_i > 0u {
                match_i = 0u;
                i = match_start + 1u;
            } else {
                i += 1u;
            }
        }
    }
}

fn iter_between_matches(s: &'a str, sep: &'b str, f: &fn(uint, uint)) {
    let mut last_end = 0u;
    do iter_matches(s, sep) |from, to| {
        f(last_end, from);
        last_end = to;
    }
    f(last_end, len(s));
}

/**
 * Splits a string into a vector of the substrings separated by a given string
 *
 * # Example
 *
 * ~~~
 * fail_unless!(["", "XXX", "YYY", ""] == split_str(".XXX.YYY.", "."))
 * ~~~
 */
pub fn split_str(s: &'a str, sep: &'b str) -> ~[~str] {
    let mut result = ~[];
    do iter_between_matches(s, sep) |from, to| {
        unsafe { result.push(raw::slice_bytes_unique(s, from, to)); }
    }
    result
}

pub fn split_str_nonempty(s: &'a str, sep: &'b str) -> ~[~str] {
    let mut result = ~[];
    do iter_between_matches(s, sep) |from, to| {
        if to > from {
            unsafe { result.push(raw::slice_bytes_unique(s, from, to)); }
        }
    }
    result
}

/// Levenshtein Distance between two strings
pub fn levdistance(s: &str, t: &str) -> uint {

    let slen = s.len();
    let tlen = t.len();

    if slen == 0 { return tlen; }
    if tlen == 0 { return slen; }

    let mut dcol = vec::from_fn(tlen + 1, |x| x);

    for s.each_chari |i, sc| {

        let mut current = i;
        dcol[0] = current + 1;

        for t.each_chari |j, tc| {

            let mut next = dcol[j + 1];

            if sc == tc {
                dcol[j + 1] = current;
            } else {
                dcol[j + 1] = ::cmp::min(current, next);
                dcol[j + 1] = ::cmp::min(dcol[j + 1], dcol[j]) + 1;
            }

            current = next;
        }
    }

    return dcol[tlen];
}

/**
 * Splits a string into a vector of the substrings separated by LF ('\n').
 */
pub fn lines(s: &str) -> ~[~str] {
    split_char_no_trailing(s, '\n')
}

/**
 * Splits a string into a vector of the substrings separated by LF ('\n')
 * and/or CR LF ("\r\n")
 */
pub fn lines_any(s: &str) -> ~[~str] {
    vec::map(lines(s), |s| {
        let l = len(*s);
        let mut cp = copy *s;
        if l > 0u && s[l - 1u] == '\r' as u8 {
            unsafe { raw::set_len(&mut cp, l - 1u); }
        }
        cp
    })
}

/// Splits a string into a vector of the substrings separated by whitespace
pub fn words(s: &str) -> ~[~str] {
    split_nonempty(s, char::is_whitespace)
}

/** Split a string into a vector of substrings,
 *  each of which is less than a limit
 */
pub fn split_within(ss: &str, lim: uint) -> ~[~str] {
    let words = str::words(ss);

    // empty?
    if words == ~[] { return ~[]; }

    let mut rows : ~[~str] = ~[];
    let mut row  : ~str    = ~"";

    for words.each |wptr| {
        let word = copy *wptr;

        // if adding this word to the row would go over the limit,
        // then start a new row
        if row.len() + word.len() + 1 > lim {
            rows.push(copy row); // save previous row
            row = word;    // start a new one
        } else {
            if row.len() > 0 { row += ~" " } // separate words
            row += word;  // append to this row
        }
    }

    // save the last row
    if row != ~"" { rows.push(row); }

    rows
}



/// Convert a string to lowercase. ASCII only
pub fn to_lower(s: &str) -> ~str {
    map(s,
        |c| unsafe{(libc::tolower(c as libc::c_char)) as char}
    )
}

/// Convert a string to uppercase. ASCII only
pub fn to_upper(s: &str) -> ~str {
    map(s,
        |c| unsafe{(libc::toupper(c as libc::c_char)) as char}
    )
}

/**
 * Replace all occurrences of one string with another
 *
 * # Arguments
 *
 * * s - The string containing substrings to replace
 * * from - The string to replace
 * * to - The replacement string
 *
 * # Return value
 *
 * The original string with all occurances of `from` replaced with `to`
 */
pub fn replace(s: &str, from: &str, to: &str) -> ~str {
    let mut result = ~"", first = true;
    do iter_between_matches(s, from) |start, end| {
        if first {
            first = false;
        } else {
            unsafe { push_str(&mut result, to); }
        }
        unsafe { push_str(&mut result, raw::slice_bytes_unique(s, start, end)); }
    }
    result
}

/*
Section: Comparing strings
*/

/// Bytewise slice equality
#[cfg(notest)]
#[lang="str_eq"]
pub fn eq_slice(a: &str, b: &str) -> bool {
    do as_buf(a) |ap, alen| {
        do as_buf(b) |bp, blen| {
            if (alen != blen) { false }
            else {
                unsafe {
                    libc::memcmp(ap as *libc::c_void,
                                 bp as *libc::c_void,
                                 (alen - 1) as libc::size_t) == 0
                }
            }
        }
    }
}

#[cfg(test)]
pub fn eq_slice(a: &str, b: &str) -> bool {
    do as_buf(a) |ap, alen| {
        do as_buf(b) |bp, blen| {
            if (alen != blen) { false }
            else {
                unsafe {
                    libc::memcmp(ap as *libc::c_void,
                                 bp as *libc::c_void,
                                 (alen - 1) as libc::size_t) == 0
                }
            }
        }
    }
}

/// Bytewise string equality
#[cfg(notest)]
#[lang="uniq_str_eq"]
pub fn eq(a: &~str, b: &~str) -> bool {
    eq_slice(*a, *b)
}

#[cfg(test)]
pub fn eq(a: &~str, b: &~str) -> bool {
    eq_slice(*a, *b)
}

fn cmp(a: &str, b: &str) -> Ordering {
    let low = uint::min(a.len(), b.len());

    for uint::range(0, low) |idx| {
        match a[idx].cmp(&b[idx]) {
          Greater => return Greater,
          Less => return Less,
          Equal => ()
        }
    }

    a.len().cmp(&b.len())
}

#[cfg(notest)]
impl TotalOrd for &'self str {
    fn cmp(&self, other: & &'self str) -> Ordering { cmp(*self, *other) }
}

#[cfg(notest)]
impl TotalOrd for ~str {
    fn cmp(&self, other: &~str) -> Ordering { cmp(*self, *other) }
}

#[cfg(notest)]
impl TotalOrd for @str {
    fn cmp(&self, other: &@str) -> Ordering { cmp(*self, *other) }
}

/// Bytewise slice less than
fn lt(a: &str, b: &str) -> bool {
    let (a_len, b_len) = (a.len(), b.len());
    let mut end = uint::min(a_len, b_len);

    let mut i = 0;
    while i < end {
        let (c_a, c_b) = (a[i], b[i]);
        if c_a < c_b { return true; }
        if c_a > c_b { return false; }
        i += 1;
    }

    return a_len < b_len;
}

/// Bytewise less than or equal
pub fn le(a: &str, b: &str) -> bool {
    !lt(b, a)
}

/// Bytewise greater than or equal
fn ge(a: &str, b: &str) -> bool {
    !lt(a, b)
}

/// Bytewise greater than
fn gt(a: &str, b: &str) -> bool {
    !le(a, b)
}

#[cfg(notest)]
impl Eq for &'self str {
    #[inline(always)]
    fn eq(&self, other: & &'self str) -> bool {
        eq_slice((*self), (*other))
    }
    #[inline(always)]
    fn ne(&self, other: & &'self str) -> bool { !(*self).eq(other) }
}

#[cfg(notest)]
impl Eq for ~str {
    #[inline(always)]
    fn eq(&self, other: &~str) -> bool {
        eq_slice((*self), (*other))
    }
    #[inline(always)]
    fn ne(&self, other: &~str) -> bool { !(*self).eq(other) }
}

#[cfg(notest)]
impl Eq for @str {
    #[inline(always)]
    fn eq(&self, other: &@str) -> bool {
        eq_slice((*self), (*other))
    }
    #[inline(always)]
    fn ne(&self, other: &@str) -> bool { !(*self).eq(other) }
}

#[cfg(notest)]
impl Ord for ~str {
    #[inline(always)]
    fn lt(&self, other: &~str) -> bool { lt((*self), (*other)) }
    #[inline(always)]
    fn le(&self, other: &~str) -> bool { le((*self), (*other)) }
    #[inline(always)]
    fn ge(&self, other: &~str) -> bool { ge((*self), (*other)) }
    #[inline(always)]
    fn gt(&self, other: &~str) -> bool { gt((*self), (*other)) }
}

#[cfg(notest)]
impl Ord for &'self str {
    #[inline(always)]
    fn lt(&self, other: & &'self str) -> bool { lt((*self), (*other)) }
    #[inline(always)]
    fn le(&self, other: & &'self str) -> bool { le((*self), (*other)) }
    #[inline(always)]
    fn ge(&self, other: & &'self str) -> bool { ge((*self), (*other)) }
    #[inline(always)]
    fn gt(&self, other: & &'self str) -> bool { gt((*self), (*other)) }
}

#[cfg(notest)]
impl Ord for @str {
    #[inline(always)]
    fn lt(&self, other: &@str) -> bool { lt((*self), (*other)) }
    #[inline(always)]
    fn le(&self, other: &@str) -> bool { le((*self), (*other)) }
    #[inline(always)]
    fn ge(&self, other: &@str) -> bool { ge((*self), (*other)) }
    #[inline(always)]
    fn gt(&self, other: &@str) -> bool { gt((*self), (*other)) }
}

#[cfg(notest)]
impl Equiv<~str> for &'self str {
    #[inline(always)]
    fn equiv(&self, other: &~str) -> bool { eq_slice(*self, *other) }
}

/*
Section: Iterating through strings
*/

/**
 * Return true if a predicate matches all characters or if the string
 * contains no characters
 */
pub fn all(s: &str, it: &fn(char) -> bool) -> bool {
    all_between(s, 0u, len(s), it)
}

/**
 * Return true if a predicate matches any character (and false if it
 * matches none or there are no characters)
 */
pub fn any(ss: &str, pred: &fn(char) -> bool) -> bool {
    !all(ss, |cc| !pred(cc))
}

/// Apply a function to each character
pub fn map(ss: &str, ff: &fn(char) -> char) -> ~str {
    let mut result = ~"";
    unsafe {
        reserve(&mut result, len(ss));
        for ss.each_char |cc| {
            str::push_char(&mut result, ff(cc));
        }
    }
    result
}

/// Iterate over the bytes in a string
#[inline(always)]
pub fn each(s: &str, it: &fn(u8) -> bool) {
    eachi(s, |_i, b| it(b))
}

/// Iterate over the bytes in a string, with indices
#[inline(always)]
pub fn eachi(s: &str, it: &fn(uint, u8) -> bool) {
    let mut pos = 0;
    let len = s.len();

    while pos < len {
        if !it(pos, s[pos]) { break; }
        pos += 1;
    }
}

/// Iterate over the bytes in a string in reverse
#[inline(always)]
pub fn each_reverse(s: &str, it: &fn(u8) -> bool) {
    eachi_reverse(s, |_i, b| it(b) )
}

/// Iterate over the bytes in a string in reverse, with indices
#[inline(always)]
pub fn eachi_reverse(s: &str, it: &fn(uint, u8) -> bool) {
    let mut pos = s.len();
    while pos > 0 {
        pos -= 1;
        if !it(pos, s[pos]) { break; }
    }
}

/// Iterates over the chars in a string
#[inline(always)]
pub fn each_char(s: &str, it: &fn(char) -> bool) {
    each_chari(s, |_i, c| it(c))
}

/// Iterates over the chars in a string, with indices
#[inline(always)]
pub fn each_chari(s: &str, it: &fn(uint, char) -> bool) {
    let mut pos = 0;
    let mut ch_pos = 0u;
    let len = s.len();
    while pos < len {
        let CharRange {ch, next} = char_range_at(s, pos);
        pos = next;
        if !it(ch_pos, ch) { break; }
        ch_pos += 1u;
    }
}

/// Iterates over the chars in a string in reverse
#[inline(always)]
pub fn each_char_reverse(s: &str, it: &fn(char) -> bool) {
    let mut pos = 0;
    let len = s.char_len();
    while pos > 0 {
        let CharRange {ch, next} = char_range_at_reverse(s, pos);
        pos = next;
        if !it(ch) { break; }
    }
}

// Iterates over the chars in a string in reverse, with indices
#[inline(always)]
pub fn each_chari_reverse(s: &str, it: &fn(uint, char) -> bool) {
    let mut ch_pos = s.char_len();
    for s.each_char_reverse |ch| {
        ch_pos -= 1;
        if !it(ch_pos, ch) { break; }
    }
}

/// Apply a function to each substring after splitting by character
pub fn split_char_each(ss: &str, cc: char, ff: &fn(v: &str) -> bool) {
    vec::each(split_char(ss, cc), |s| ff(*s))
}

/**
 * Apply a function to each substring after splitting by character, up to
 * `count` times
 */
pub fn splitn_char_each(ss: &str, sep: char, count: uint,
                         ff: &fn(v: &str) -> bool) {
    vec::each(splitn_char(ss, sep, count), |s| ff(*s))
}

/// Apply a function to each word
pub fn words_each(ss: &str, ff: &fn(v: &str) -> bool) {
    vec::each(words(ss), |s| ff(*s))
}

/**
 * Apply a function to each line (by '\n')
 */
pub fn lines_each(ss: &str, ff: &fn(v: &str) -> bool) {
    vec::each(lines(ss), |s| ff(*s))
}

/*
Section: Searching
*/

/**
 * Returns the byte index of the first matching character
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `c` - The character to search for
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching character
 * or `none` if there is no match
 */
pub fn find_char(s: &str, c: char) -> Option<uint> {
    find_char_between(s, c, 0u, len(s))
}

/**
 * Returns the byte index of the first matching character beginning
 * from a given byte offset
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `c` - The character to search for
 * * `start` - The byte index to begin searching at, inclusive
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `len(s)`. `start` must be the
 * index of a character boundary, as defined by `is_char_boundary`.
 */
pub fn find_char_from(s: &str, c: char, start: uint) -> Option<uint> {
    find_char_between(s, c, start, len(s))
}

/**
 * Returns the byte index of the first matching character within a given range
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `c` - The character to search for
 * * `start` - The byte index to begin searching at, inclusive
 * * `end` - The byte index to end searching at, exclusive
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `end` and `end` must be less than
 * or equal to `len(s)`. `start` must be the index of a character boundary,
 * as defined by `is_char_boundary`.
 */
pub fn find_char_between(s: &str, c: char, start: uint, end: uint)
    -> Option<uint> {
    if c < 128u as char {
        fail_unless!(start <= end);
        fail_unless!(end <= len(s));
        let mut i = start;
        let b = c as u8;
        while i < end {
            if s[i] == b { return Some(i); }
            i += 1u;
        }
        return None;
    } else {
        find_between(s, start, end, |x| x == c)
    }
}

/**
 * Returns the byte index of the last matching character
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `c` - The character to search for
 *
 * # Return value
 *
 * An `option` containing the byte index of the last matching character
 * or `none` if there is no match
 */
pub fn rfind_char(s: &str, c: char) -> Option<uint> {
    rfind_char_between(s, c, len(s), 0u)
}

/**
 * Returns the byte index of the last matching character beginning
 * from a given byte offset
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `c` - The character to search for
 * * `start` - The byte index to begin searching at, exclusive
 *
 * # Return value
 *
 * An `option` containing the byte index of the last matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `len(s)`. `start` must be
 * the index of a character boundary, as defined by `is_char_boundary`.
 */
pub fn rfind_char_from(s: &str, c: char, start: uint) -> Option<uint> {
    rfind_char_between(s, c, start, 0u)
}

/**
 * Returns the byte index of the last matching character within a given range
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `c` - The character to search for
 * * `start` - The byte index to begin searching at, exclusive
 * * `end` - The byte index to end searching at, inclusive
 *
 * # Return value
 *
 * An `option` containing the byte index of the last matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `end` must be less than or equal to `start` and `start` must be less than
 * or equal to `len(s)`. `start` must be the index of a character boundary,
 * as defined by `is_char_boundary`.
 */
pub fn rfind_char_between(s: &str, c: char, start: uint, end: uint)
    -> Option<uint> {
    if c < 128u as char {
        fail_unless!(start >= end);
        fail_unless!(start <= len(s));
        let mut i = start;
        let b = c as u8;
        while i > end {
            i -= 1u;
            if s[i] == b { return Some(i); }
        }
        return None;
    } else {
        rfind_between(s, start, end, |x| x == c)
    }
}

/**
 * Returns the byte index of the first character that satisfies
 * the given predicate
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `f` - The predicate to satisfy
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching character
 * or `none` if there is no match
 */
pub fn find(s: &str, f: &fn(char) -> bool) -> Option<uint> {
    find_between(s, 0u, len(s), f)
}

/**
 * Returns the byte index of the first character that satisfies
 * the given predicate, beginning from a given byte offset
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `start` - The byte index to begin searching at, inclusive
 * * `f` - The predicate to satisfy
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching charactor
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `len(s)`. `start` must be the
 * index of a character boundary, as defined by `is_char_boundary`.
 */
pub fn find_from(s: &str, start: uint, f: &fn(char)
    -> bool) -> Option<uint> {
    find_between(s, start, len(s), f)
}

/**
 * Returns the byte index of the first character that satisfies
 * the given predicate, within a given range
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `start` - The byte index to begin searching at, inclusive
 * * `end` - The byte index to end searching at, exclusive
 * * `f` - The predicate to satisfy
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `end` and `end` must be less than
 * or equal to `len(s)`. `start` must be the index of a character
 * boundary, as defined by `is_char_boundary`.
 */
pub fn find_between(s: &str,
                         start: uint,
                         end: uint,
                         f: &fn(char) -> bool)
                      -> Option<uint> {
    fail_unless!(start <= end);
    fail_unless!(end <= len(s));
    fail_unless!(is_char_boundary(s, start));
    let mut i = start;
    while i < end {
        let CharRange {ch, next} = char_range_at(s, i);
        if f(ch) { return Some(i); }
        i = next;
    }
    return None;
}

/**
 * Returns the byte index of the last character that satisfies
 * the given predicate
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `f` - The predicate to satisfy
 *
 * # Return value
 *
 * An option containing the byte index of the last matching character
 * or `none` if there is no match
 */
pub fn rfind(s: &str, f: &fn(char) -> bool) -> Option<uint> {
    rfind_between(s, len(s), 0u, f)
}

/**
 * Returns the byte index of the last character that satisfies
 * the given predicate, beginning from a given byte offset
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `start` - The byte index to begin searching at, exclusive
 * * `f` - The predicate to satisfy
 *
 * # Return value
 *
 * An `option` containing the byte index of the last matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `len(s)', `start` must be the
 * index of a character boundary, as defined by `is_char_boundary`
 */
pub fn rfind_from(s: &str, start: uint, f: &fn(char) -> bool)
    -> Option<uint> {
    rfind_between(s, start, 0u, f)
}

/**
 * Returns the byte index of the last character that satisfies
 * the given predicate, within a given range
 *
 * # Arguments
 *
 * * `s` - The string to search
 * * `start` - The byte index to begin searching at, exclusive
 * * `end` - The byte index to end searching at, inclusive
 * * `f` - The predicate to satisfy
 *
 * # Return value
 *
 * An `option` containing the byte index of the last matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `end` must be less than or equal to `start` and `start` must be less
 * than or equal to `len(s)`. `start` must be the index of a character
 * boundary, as defined by `is_char_boundary`
 */
pub fn rfind_between(s: &str, start: uint, end: uint,
                          f: &fn(char) -> bool)
    -> Option<uint> {
    fail_unless!(start >= end);
    fail_unless!(start <= len(s));
    fail_unless!(is_char_boundary(s, start));
    let mut i = start;
    while i > end {
        let CharRange {ch, next: prev} = char_range_at_reverse(s, i);
        if f(ch) { return Some(prev); }
        i = prev;
    }
    return None;
}

// Utility used by various searching functions
fn match_at(haystack: &'a str, needle: &'b str, at: uint) -> bool {
    let mut i = at;
    for each(needle) |c| { if haystack[i] != c { return false; } i += 1u; }
    return true;
}

/**
 * Returns the byte index of the first matching substring
 *
 * # Arguments
 *
 * * `haystack` - The string to search
 * * `needle` - The string to search for
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching substring
 * or `none` if there is no match
 */
pub fn find_str(haystack: &'a str, needle: &'b str) -> Option<uint> {
    find_str_between(haystack, needle, 0u, len(haystack))
}

/**
 * Returns the byte index of the first matching substring beginning
 * from a given byte offset
 *
 * # Arguments
 *
 * * `haystack` - The string to search
 * * `needle` - The string to search for
 * * `start` - The byte index to begin searching at, inclusive
 *
 * # Return value
 *
 * An `option` containing the byte index of the last matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `len(s)`
 */
pub fn find_str_from(haystack: &'a str, needle: &'b str, start: uint)
  -> Option<uint> {
    find_str_between(haystack, needle, start, len(haystack))
}

/**
 * Returns the byte index of the first matching substring within a given range
 *
 * # Arguments
 *
 * * `haystack` - The string to search
 * * `needle` - The string to search for
 * * `start` - The byte index to begin searching at, inclusive
 * * `end` - The byte index to end searching at, exclusive
 *
 * # Return value
 *
 * An `option` containing the byte index of the first matching character
 * or `none` if there is no match
 *
 * # Failure
 *
 * `start` must be less than or equal to `end` and `end` must be less than
 * or equal to `len(s)`.
 */
pub fn find_str_between(haystack: &'a str, needle: &'b str, start: uint,
                         end:uint)
  -> Option<uint> {
    // See Issue #1932 for why this is a naive search
    fail_unless!(end <= len(haystack));
    let needle_len = len(needle);
    if needle_len == 0u { return Some(start); }
    if needle_len > end { return None; }

    let mut i = start;
    let e = end - needle_len;
    while i <= e {
        if match_at(haystack, needle, i) { return Some(i); }
        i += 1u;
    }
    return None;
}

/**
 * Returns true if one string contains another
 *
 * # Arguments
 *
 * * haystack - The string to look in
 * * needle - The string to look for
 */
pub fn contains(haystack: &'a str, needle: &'b str) -> bool {
    find_str(haystack, needle).is_some()
}

/**
 * Returns true if a string contains a char.
 *
 * # Arguments
 *
 * * haystack - The string to look in
 * * needle - The char to look for
 */
pub fn contains_char(haystack: &str, needle: char) -> bool {
    find_char(haystack, needle).is_some()
}

/**
 * Returns true if one string starts with another
 *
 * # Arguments
 *
 * * haystack - The string to look in
 * * needle - The string to look for
 */
pub fn starts_with(haystack: &'a str, needle: &'b str) -> bool {
    let haystack_len = len(haystack), needle_len = len(needle);
    if needle_len == 0u { true }
    else if needle_len > haystack_len { false }
    else { match_at(haystack, needle, 0u) }
}

/**
 * Returns true if one string ends with another
 *
 * # Arguments
 *
 * * haystack - The string to look in
 * * needle - The string to look for
 */
pub fn ends_with(haystack: &'a str, needle: &'b str) -> bool {
    let haystack_len = len(haystack), needle_len = len(needle);
    if needle_len == 0u { true }
    else if needle_len > haystack_len { false }
    else { match_at(haystack, needle, haystack_len - needle_len) }
}

/*
Section: String properties
*/

/// Determines if a string contains only ASCII characters
pub fn is_ascii(s: &str) -> bool {
    let mut i: uint = len(s);
    while i > 0u { i -= 1u; if !u8::is_ascii(s[i]) { return false; } }
    return true;
}

/// Returns true if the string has length 0
pub fn is_empty(s: &str) -> bool { len(s) == 0u }

/**
 * Returns true if the string contains only whitespace
 *
 * Whitespace characters are determined by `char::is_whitespace`
 */
pub fn is_whitespace(s: &str) -> bool {
    return all(s, char::is_whitespace);
}

/**
 * Returns true if the string contains only alphanumerics
 *
 * Alphanumeric characters are determined by `char::is_alphanumeric`
 */
fn is_alphanumeric(s: &str) -> bool {
    return all(s, char::is_alphanumeric);
}

/// Returns the string length/size in bytes not counting the null terminator
pub fn len(s: &str) -> uint {
    do as_buf(s) |_p, n| { n - 1u }
}

/// Returns the number of characters that a string holds
pub fn char_len(s: &str) -> uint { count_chars(s, 0u, len(s)) }

/*
Section: Misc
*/

/// Determines if a vector of bytes contains valid UTF-8
pub fn is_utf8(v: &[const u8]) -> bool {
    let mut i = 0u;
    let total = vec::len::<u8>(v);
    while i < total {
        let mut chsize = utf8_char_width(v[i]);
        if chsize == 0u { return false; }
        if i + chsize > total { return false; }
        i += 1u;
        while chsize > 1u {
            if v[i] & 192u8 != tag_cont_u8 { return false; }
            i += 1u;
            chsize -= 1u;
        }
    }
    return true;
}

/// Determines if a vector of `u16` contains valid UTF-16
pub fn is_utf16(v: &[u16]) -> bool {
    let len = vec::len(v);
    let mut i = 0u;
    while (i < len) {
        let u = v[i];

        if  u <= 0xD7FF_u16 || u >= 0xE000_u16 {
            i += 1u;

        } else {
            if i+1u < len { return false; }
            let u2 = v[i+1u];
            if u < 0xD7FF_u16 || u > 0xDBFF_u16 { return false; }
            if u2 < 0xDC00_u16 || u2 > 0xDFFF_u16 { return false; }
            i += 2u;
        }
    }
    return true;
}

/// Converts to a vector of `u16` encoded as UTF-16
pub fn to_utf16(s: &str) -> ~[u16] {
    let mut u = ~[];
    for s.each_char |ch| {
        // Arithmetic with u32 literals is easier on the eyes than chars.
        let mut ch = ch as u32;

        unsafe {
            if (ch & 0xFFFF_u32) == ch {
                // The BMP falls through (assuming non-surrogate, as it
                // should)
                fail_unless!(ch <= 0xD7FF_u32 || ch >= 0xE000_u32);
                u.push(ch as u16)
            } else {
                // Supplementary planes break into surrogates.
                fail_unless!(ch >= 0x1_0000_u32 && ch <= 0x10_FFFF_u32);
                ch -= 0x1_0000_u32;
                let w1 = 0xD800_u16 | ((ch >> 10) as u16);
                let w2 = 0xDC00_u16 | ((ch as u16) & 0x3FF_u16);
                u.push_all(~[w1, w2])
            }
        }
    }
    u
}

pub fn utf16_chars(v: &[u16], f: &fn(char)) {
    let len = vec::len(v);
    let mut i = 0u;
    while (i < len && v[i] != 0u16) {
        let mut u = v[i];

        if  u <= 0xD7FF_u16 || u >= 0xE000_u16 {
            f(u as char);
            i += 1u;

        } else {
            let u2 = v[i+1u];
            fail_unless!(u >= 0xD800_u16 && u <= 0xDBFF_u16);
            fail_unless!(u2 >= 0xDC00_u16 && u2 <= 0xDFFF_u16);
            let mut c = (u - 0xD800_u16) as char;
            c = c << 10;
            c |= (u2 - 0xDC00_u16) as char;
            c |= 0x1_0000_u32 as char;
            f(c);
            i += 2u;
        }
    }
}


pub fn from_utf16(v: &[u16]) -> ~str {
    let mut buf = ~"";
    unsafe {
        reserve(&mut buf, vec::len(v));
        utf16_chars(v, |ch| push_char(&mut buf, ch));
    }
    buf
}

pub fn with_capacity(capacity: uint) -> ~str {
    let mut buf = ~"";
    unsafe { reserve(&mut buf, capacity); }
    buf
}

/**
 * As char_len but for a slice of a string
 *
 * # Arguments
 *
 * * s - A valid string
 * * start - The position inside `s` where to start counting in bytes
 * * end - The position where to stop counting
 *
 * # Return value
 *
 * The number of Unicode characters in `s` between the given indices.
 */
pub fn count_chars(s: &str, start: uint, end: uint) -> uint {
    fail_unless!(is_char_boundary(s, start));
    fail_unless!(is_char_boundary(s, end));
    let mut i = start, len = 0u;
    while i < end {
        let next = char_range_at(s, i).next;
        len += 1u;
        i = next;
    }
    return len;
}

/// Counts the number of bytes taken by the `n` in `s` starting from `start`.
pub fn count_bytes(s: &'b str, start: uint, n: uint) -> uint {
    fail_unless!(is_char_boundary(s, start));
    let mut end = start, cnt = n;
    let l = len(s);
    while cnt > 0u {
        fail_unless!(end < l);
        let next = char_range_at(s, end).next;
        cnt -= 1u;
        end = next;
    }
    end - start
}

/// Given a first byte, determine how many bytes are in this UTF-8 character
pub fn utf8_char_width(b: u8) -> uint {
    let byte: uint = b as uint;
    if byte < 128u { return 1u; }
    // Not a valid start byte
    if byte < 192u { return 0u; }
    if byte < 224u { return 2u; }
    if byte < 240u { return 3u; }
    if byte < 248u { return 4u; }
    if byte < 252u { return 5u; }
    return 6u;
}

/**
 * Returns false if the index points into the middle of a multi-byte
 * character sequence.
 */
pub fn is_char_boundary(s: &str, index: uint) -> bool {
    if index == len(s) { return true; }
    let b = s[index];
    return b < 128u8 || b >= 192u8;
}

/**
 * Pluck a character out of a string and return the index of the next
 * character.
 *
 * This function can be used to iterate over the unicode characters of a
 * string.
 *
 * # Example
 *
 * ~~~
 * let s = "中华Việt Nam";
 * let i = 0u;
 * while i < str::len(s) {
 *     let CharRange {ch, next} = str::char_range_at(s, i);
 *     std::io::println(fmt!("%u: %c",i,ch));
 *     i = next;
 * }
 * ~~~
 *
 * # Example output
 *
 * ~~~
 * 0: 中
 * 3: 华
 * 6: V
 * 7: i
 * 8: ệ
 * 11: t
 * 12:
 * 13: N
 * 14: a
 * 15: m
 * ~~~
 *
 * # Arguments
 *
 * * s - The string
 * * i - The byte offset of the char to extract
 *
 * # Return value
 *
 * A record {ch: char, next: uint} containing the char value and the byte
 * index of the next unicode character.
 *
 * # Failure
 *
 * If `i` is greater than or equal to the length of the string.
 * If `i` is not the index of the beginning of a valid UTF-8 character.
 */
pub fn char_range_at(s: &str, i: uint) -> CharRange {
    let b0 = s[i];
    let w = utf8_char_width(b0);
    fail_unless!((w != 0u));
    if w == 1u { return CharRange {ch: b0 as char, next: i + 1u}; }
    let mut val = 0u;
    let end = i + w;
    let mut i = i + 1u;
    while i < end {
        let byte = s[i];
        fail_unless!((byte & 192u8 == tag_cont_u8));
        val <<= 6u;
        val += (byte & 63u8) as uint;
        i += 1u;
    }
    // Clunky way to get the right bits from the first byte. Uses two shifts,
    // the first to clip off the marker bits at the left of the byte, and then
    // a second (as uint) to get it to the right position.
    val += ((b0 << ((w + 1u) as u8)) as uint) << ((w - 1u) * 6u - w - 1u);
    return CharRange {ch: val as char, next: i};
}

/// Plucks the `n`th character from the beginning of a string
pub fn char_at(s: &str, i: uint) -> char {
    return char_range_at(s, i).ch;
}

pub struct CharRange {
    ch: char,
    next: uint
}

/**
 * Given a byte position and a str, return the previous char and its position
 *
 * This function can be used to iterate over a unicode string in reverse.
 */
fn char_range_at_reverse(ss: &str, start: uint) -> CharRange {
    let mut prev = start;

    // while there is a previous byte == 10......
    while prev > 0u && ss[prev - 1u] & 192u8 == tag_cont_u8 {
        prev -= 1u;
    }

    // now refer to the initial byte of previous char
    prev -= 1u;

    let ch = char_at(ss, prev);
    return CharRange {ch:ch, next:prev};
}

/// Plucks the `n`th character from the end of a string
pub fn char_at_reverse(s: &str, i: uint) -> char {
    char_range_at_reverse(s, i).ch
}

/**
 * Loop through a substring, char by char
 *
 * # Safety note
 *
 * * This function does not check whether the substring is valid.
 * * This function fails if `start` or `end` do not
 *   represent valid positions inside `s`
 *
 * # Arguments
 *
 * * s - A string to traverse. It may be empty.
 * * start - The byte offset at which to start in the string.
 * * end - The end of the range to traverse
 * * it - A block to execute with each consecutive character of `s`.
 *        Return `true` to continue, `false` to stop.
 *
 * # Return value
 *
 * `true` If execution proceeded correctly, `false` if it was interrupted,
 * that is if `it` returned `false` at any point.
 */
pub fn all_between(s: &str, start: uint, end: uint,
                    it: &fn(char) -> bool) -> bool {
    fail_unless!(is_char_boundary(s, start));
    let mut i = start;
    while i < end {
        let CharRange {ch, next} = char_range_at(s, i);
        if !it(ch) { return false; }
        i = next;
    }
    return true;
}

/**
 * Loop through a substring, char by char
 *
 * # Safety note
 *
 * * This function does not check whether the substring is valid.
 * * This function fails if `start` or `end` do not
 *   represent valid positions inside `s`
 *
 * # Arguments
 *
 * * s - A string to traverse. It may be empty.
 * * start - The byte offset at which to start in the string.
 * * end - The end of the range to traverse
 * * it - A block to execute with each consecutive character of `s`.
 *        Return `true` to continue, `false` to stop.
 *
 * # Return value
 *
 * `true` if `it` returns `true` for any character
 */
pub fn any_between(s: &str, start: uint, end: uint,
                    it: &fn(char) -> bool) -> bool {
    !all_between(s, start, end, |c| !it(c))
}

// UTF-8 tags and ranges
static tag_cont_u8: u8 = 128u8;
static tag_cont: uint = 128u;
static max_one_b: uint = 128u;
static tag_two_b: uint = 192u;
static max_two_b: uint = 2048u;
static tag_three_b: uint = 224u;
static max_three_b: uint = 65536u;
static tag_four_b: uint = 240u;
static max_four_b: uint = 2097152u;
static tag_five_b: uint = 248u;
static max_five_b: uint = 67108864u;
static tag_six_b: uint = 252u;

// Constants used for converting strs to floats
pub static inf_buf: [u8*3] = ['i' as u8, 'n' as u8, 'f' as u8];
pub static positive_inf_buf: [u8*4] = ['+' as u8, 'i' as u8,
                                       'n' as u8, 'f' as u8];
pub static negative_inf_buf: [u8*4] = ['-' as u8, 'i' as u8,
                                       'n' as u8, 'f' as u8];
pub static nan_buf: [u8*3] = ['N' as u8, 'a' as u8, 'N' as u8];

/**
 * Work with the byte buffer of a string.
 *
 * Allows for unsafe manipulation of strings, which is useful for foreign
 * interop.
 *
 * # Example
 *
 * ~~~
 * let i = str::as_bytes("Hello World") { |bytes| vec::len(bytes) };
 * ~~~
 */
pub fn as_bytes<T>(s: &const ~str, f: &fn(&~[u8]) -> T) -> T {
    unsafe {
        let v: *~[u8] = cast::transmute(copy s);
        f(&*v)
    }
}

/**
 * Work with the byte buffer of a string as a byte slice.
 *
 * The byte slice does not include the null terminator.
 */
pub fn as_bytes_slice(s: &'a str) -> &'a [u8] {
    unsafe {
        let (ptr, len): (*u8, uint) = ::cast::reinterpret_cast(&s);
        let outgoing_tuple: (*u8, uint) = (ptr, len - 1);
        return ::cast::reinterpret_cast(&outgoing_tuple);
    }
}

/**
 * Work with the byte buffer of a string as a null-terminated C string.
 *
 * Allows for unsafe manipulation of strings, which is useful for foreign
 * interop. This is similar to `str::as_buf`, but guarantees null-termination.
 * If the given slice is not already null-terminated, this function will
 * allocate a temporary, copy the slice, null terminate it, and pass
 * that instead.
 *
 * # Example
 *
 * ~~~
 * let s = str::as_c_str("PATH", { |path| libc::getenv(path) });
 * ~~~
 */
pub fn as_c_str<T>(s: &str, f: &fn(*libc::c_char) -> T) -> T {
    do as_buf(s) |buf, len| {
        // NB: len includes the trailing null.
        fail_unless!(len > 0);
        if unsafe { *(ptr::offset(buf,len-1)) != 0 } {
            as_c_str(from_slice(s), f)
        } else {
            f(buf as *libc::c_char)
        }
    }
}


/**
 * Work with the byte buffer and length of a slice.
 *
 * The given length is one byte longer than the 'official' indexable
 * length of the string. This is to permit probing the byte past the
 * indexable area for a null byte, as is the case in slices pointing
 * to full strings, or suffixes of them.
 */
#[inline(always)]
pub fn as_buf<T>(s: &str, f: &fn(*u8, uint) -> T) -> T {
    unsafe {
        let v : *(*u8,uint) = ::cast::reinterpret_cast(&ptr::addr_of(&s));
        let (buf,len) = *v;
        f(buf, len)
    }
}

/**
 * Reserves capacity for exactly `n` bytes in the given string, not including
 * the null terminator.
 *
 * Assuming single-byte characters, the resulting string will be large
 * enough to hold a string of length `n`. To account for the null terminator,
 * the underlying buffer will have the size `n` + 1.
 *
 * If the capacity for `s` is already equal to or greater than the requested
 * capacity, then no action is taken.
 *
 * # Arguments
 *
 * * s - A string
 * * n - The number of bytes to reserve space for
 */
pub fn reserve(s: &mut ~str, n: uint) {
    unsafe {
        let v: *mut ~[u8] = cast::transmute(s);
        vec::reserve(&mut *v, n + 1);
    }
}

/**
 * Reserves capacity for at least `n` bytes in the given string, not including
 * the null terminator.
 *
 * Assuming single-byte characters, the resulting string will be large
 * enough to hold a string of length `n`. To account for the null terminator,
 * the underlying buffer will have the size `n` + 1.
 *
 * This function will over-allocate in order to amortize the allocation costs
 * in scenarios where the caller may need to repeatedly reserve additional
 * space.
 *
 * If the capacity for `s` is already equal to or greater than the requested
 * capacity, then no action is taken.
 *
 * # Arguments
 *
 * * s - A string
 * * n - The number of bytes to reserve space for
 */
pub fn reserve_at_least(s: &mut ~str, n: uint) {
    reserve(s, uint::next_power_of_two(n + 1u) - 1u)
}

/**
 * Returns the number of single-byte characters the string can hold without
 * reallocating
 */
pub fn capacity(s: &const ~str) -> uint {
    do as_bytes(s) |buf| {
        let vcap = vec::capacity(buf);
        fail_unless!(vcap > 0u);
        vcap - 1u
    }
}

/// Escape each char in `s` with char::escape_default.
pub fn escape_default(s: &str) -> ~str {
    let mut out: ~str = ~"";
    unsafe {
        reserve_at_least(&mut out, str::len(s));
        for s.each_char |c| {
            push_str(&mut out, char::escape_default(c));
        }
    }
    out
}

/// Escape each char in `s` with char::escape_unicode.
pub fn escape_unicode(s: &str) -> ~str {
    let mut out: ~str = ~"";
    unsafe {
        reserve_at_least(&mut out, str::len(s));
        for s.each_char |c| {
            push_str(&mut out, char::escape_unicode(c));
        }
    }
    out
}

/// Unsafe operations
pub mod raw {
    use cast;
    use libc;
    use ptr;
    use str::raw;
    use str::{as_buf, is_utf8, len, reserve_at_least};
    use vec;

    /// Create a Rust string from a null-terminated *u8 buffer
    pub unsafe fn from_buf(buf: *u8) -> ~str {
        let mut curr = buf, i = 0u;
        while *curr != 0u8 {
            i += 1u;
            curr = ptr::offset(buf, i);
        }
        return from_buf_len(buf, i);
    }

    /// Create a Rust string from a *u8 buffer of the given length
    pub unsafe fn from_buf_len(buf: *const u8, len: uint) -> ~str {
        let mut v: ~[u8] = vec::with_capacity(len + 1);
        vec::as_mut_buf(v, |vbuf, _len| {
            ptr::copy_memory(vbuf, buf as *u8, len)
        });
        vec::raw::set_len(&mut v, len);
        v.push(0u8);

        fail_unless!(is_utf8(v));
        return ::cast::transmute(v);
    }

    /// Create a Rust string from a null-terminated C string
    pub unsafe fn from_c_str(c_str: *libc::c_char) -> ~str {
        from_buf(::cast::reinterpret_cast(&c_str))
    }

    /// Create a Rust string from a `*c_char` buffer of the given length
    pub unsafe fn from_c_str_len(c_str: *libc::c_char, len: uint) -> ~str {
        from_buf_len(::cast::reinterpret_cast(&c_str), len)
    }

    /// Converts a vector of bytes to a string.
    pub unsafe fn from_bytes(v: &[const u8]) -> ~str {
        do vec::as_const_buf(v) |buf, len| {
            from_buf_len(buf, len)
        }
    }

    /// Converts a byte to a string.
    pub unsafe fn from_byte(u: u8) -> ~str { raw::from_bytes([u]) }

    /// Form a slice from a *u8 buffer of the given length without copying.
    pub unsafe fn buf_as_slice<T>(buf: *u8, len: uint,
                              f: &fn(v: &str) -> T) -> T {
        let v = (buf, len + 1);
        fail_unless!(is_utf8(::cast::reinterpret_cast(&v)));
        f(::cast::transmute(v))
    }

    /**
     * Takes a bytewise (not UTF-8) slice from a string.
     *
     * Returns the substring from [`begin`..`end`).
     *
     * # Failure
     *
     * If begin is greater than end.
     * If end is greater than the length of the string.
     */
    pub unsafe fn slice_bytes_unique(s: &str, begin: uint, end: uint) -> ~str {
        do as_buf(s) |sbuf, n| {
            fail_unless!((begin <= end));
            fail_unless!((end <= n));

            let mut v = vec::with_capacity(end - begin + 1u);
            unsafe {
                do vec::as_imm_buf(v) |vbuf, _vlen| {
                    let vbuf = ::cast::transmute_mut_unsafe(vbuf);
                    let src = ptr::offset(sbuf, begin);
                    ptr::copy_memory(vbuf, src, end - begin);
                }
                vec::raw::set_len(&mut v, end - begin);
                v.push(0u8);
                ::cast::transmute(v)
            }
        }
    }

    /**
     * Takes a bytewise (not UTF-8) slice from a string.
     *
     * Returns the substring from [`begin`..`end`).
     *
     * # Failure
     *
     * If begin is greater than end.
     * If end is greater than the length of the string.
     */
    #[inline]
    pub unsafe fn slice_bytes(s: &str, begin: uint, end: uint) -> &str {
        do as_buf(s) |sbuf, n| {
             fail_unless!((begin <= end));
             fail_unless!((end <= n));

             let tuple = (ptr::offset(sbuf, begin), end - begin + 1);
             ::cast::reinterpret_cast(&tuple)
        }
    }

    /// Appends a byte to a string. (Not UTF-8 safe).
    pub unsafe fn push_byte(s: &mut ~str, b: u8) {
        let new_len = s.len() + 1;
        reserve_at_least(&mut *s, new_len);
        do as_buf(*s) |buf, len| {
            let buf: *mut u8 = ::cast::reinterpret_cast(&buf);
            *ptr::mut_offset(buf, len) = b;
        }
        set_len(&mut *s, new_len);
    }

    /// Appends a vector of bytes to a string. (Not UTF-8 safe).
    unsafe fn push_bytes(s: &mut ~str, bytes: &[u8]) {
        let new_len = s.len() + bytes.len();
        reserve_at_least(&mut *s, new_len);
        for vec::each(bytes) |byte| { push_byte(&mut *s, *byte); }
    }

    /// Removes the last byte from a string and returns it. (Not UTF-8 safe).
    pub unsafe fn pop_byte(s: &mut ~str) -> u8 {
        let len = len(*s);
        fail_unless!((len > 0u));
        let b = s[len - 1u];
        unsafe { set_len(s, len - 1u) };
        return b;
    }

    /// Removes the first byte from a string and returns it. (Not UTF-8 safe).
    pub unsafe fn shift_byte(s: &mut ~str) -> u8 {
        let len = len(*s);
        fail_unless!((len > 0u));
        let b = s[0];
        *s = unsafe { raw::slice_bytes_unique(*s, 1u, len) };
        return b;
    }

    /// Sets the length of the string and adds the null terminator
    pub unsafe fn set_len(v: &mut ~str, new_len: uint) {
        let v: **vec::raw::VecRepr = cast::transmute(v);
        let repr: *vec::raw::VecRepr = *v;
        (*repr).unboxed.fill = new_len + 1u;
        let null = ptr::mut_offset(cast::transmute(&((*repr).unboxed.data)),
                                   new_len);
        *null = 0u8;
    }

    #[test]
    fn test_from_buf_len() {
        unsafe {
            let a = ~[65u8, 65u8, 65u8, 65u8, 65u8, 65u8, 65u8, 0u8];
            let b = vec::raw::to_ptr(a);
            let c = from_buf_len(b, 3u);
            fail_unless!((c == ~"AAA"));
        }
    }

}

#[cfg(notest)]
pub mod traits {
    use ops::Add;
    use str::append;

    impl Add<&'self str,~str> for ~str {
        #[inline(always)]
        fn add(&self, rhs: & &'self str) -> ~str {
            append(copy *self, (*rhs))
        }
    }
}

#[cfg(test)]
pub mod traits {}

pub trait StrSlice {
    fn all(&self, it: &fn(char) -> bool) -> bool;
    fn any(&self, it: &fn(char) -> bool) -> bool;
    fn contains(&self, needle: &'a str) -> bool;
    fn contains_char(&self, needle: char) -> bool;
    fn each(&self, it: &fn(u8) -> bool);
    fn eachi(&self, it: &fn(uint, u8) -> bool);
    fn each_reverse(&self, it: &fn(u8) -> bool);
    fn eachi_reverse(&self, it: &fn(uint, u8) -> bool);
    fn each_char(&self, it: &fn(char) -> bool);
    fn each_chari(&self, it: &fn(uint, char) -> bool);
    fn each_char_reverse(&self, it: &fn(char) -> bool);
    fn each_chari_reverse(&self, it: &fn(uint, char) -> bool);
    fn ends_with(&self, needle: &str) -> bool;
    fn is_empty(&self) -> bool;
    fn is_whitespace(&self) -> bool;
    fn is_alphanumeric(&self) -> bool;
    fn len(&self) -> uint;
    fn char_len(&self) -> uint;
    fn slice(&self, begin: uint, end: uint) -> &'self str;
    fn split(&self, sepfn: &fn(char) -> bool) -> ~[~str];
    fn split_char(&self, sep: char) -> ~[~str];
    fn split_str(&self, sep: &'a str) -> ~[~str];
    fn starts_with(&self, needle: &'a str) -> bool;
    fn substr(&self, begin: uint, n: uint) -> &'self str;
    fn to_lower(&self) -> ~str;
    fn to_upper(&self) -> ~str;
    fn escape_default(&self) -> ~str;
    fn escape_unicode(&self) -> ~str;
    fn trim(&self) -> &'self str;
    fn trim_left(&self) -> &'self str;
    fn trim_right(&self) -> &'self str;
    fn trim_chars(&self, chars_to_trim: &[char]) -> &'self str;
    fn trim_left_chars(&self, chars_to_trim: &[char]) -> &'self str;
    fn trim_right_chars(&self, chars_to_trim: &[char]) -> &'self str;
    fn to_owned(&self) -> ~str;
    fn to_managed(&self) -> @str;
    fn char_at(&self, i: uint) -> char;
    fn char_at_reverse(&self, i: uint) -> char;
    fn to_bytes(&self) -> ~[u8];
}

/// Extension methods for strings
impl StrSlice for &'self str {
    /**
     * Return true if a predicate matches all characters or if the string
     * contains no characters
     */
    #[inline]
    fn all(&self, it: &fn(char) -> bool) -> bool { all(*self, it) }
    /**
     * Return true if a predicate matches any character (and false if it
     * matches none or there are no characters)
     */
    #[inline]
    fn any(&self, it: &fn(char) -> bool) -> bool { any(*self, it) }
    /// Returns true if one string contains another
    #[inline]
    fn contains(&self, needle: &'a str) -> bool {
        contains(*self, needle)
    }
    /// Returns true if a string contains a char
    #[inline]
    fn contains_char(&self, needle: char) -> bool {
        contains_char(*self, needle)
    }
    /// Iterate over the bytes in a string
    #[inline]
    fn each(&self, it: &fn(u8) -> bool) { each(*self, it) }
    /// Iterate over the bytes in a string, with indices
    #[inline]
    fn eachi(&self, it: &fn(uint, u8) -> bool) { eachi(*self, it) }
    /// Iterate over the bytes in a string
    #[inline]
    fn each_reverse(&self, it: &fn(u8) -> bool) {
        each_reverse(*self, it)
    }
    /// Iterate over the bytes in a string, with indices
    #[inline]
    fn eachi_reverse(&self, it: &fn(uint, u8) -> bool) {
        eachi_reverse(*self, it)
    }
    /// Iterate over the chars in a string
    #[inline]
    fn each_char(&self, it: &fn(char) -> bool) { each_char(*self, it) }
    /// Iterate over the chars in a string, with indices
    #[inline]
    fn each_chari(&self, it: &fn(uint, char) -> bool) {
        each_chari(*self, it)
    }
    /// Iterate over the chars in a string in reverse
    #[inline]
    fn each_char_reverse(&self, it: &fn(char) -> bool) {
        each_char_reverse(*self, it)
    }
    /// Iterate over the chars in a string in reverse, with indices from the
    /// end
    #[inline]
    fn each_chari_reverse(&self, it: &fn(uint, char) -> bool) {
        each_chari_reverse(*self, it)
    }
    /// Returns true if one string ends with another
    #[inline]
    fn ends_with(&self, needle: &str) -> bool {
        ends_with(*self, needle)
    }
    /// Returns true if the string has length 0
    #[inline]
    fn is_empty(&self) -> bool { is_empty(*self) }
    /**
     * Returns true if the string contains only whitespace
     *
     * Whitespace characters are determined by `char::is_whitespace`
     */
    #[inline]
    fn is_whitespace(&self) -> bool { is_whitespace(*self) }
    /**
     * Returns true if the string contains only alphanumerics
     *
     * Alphanumeric characters are determined by `char::is_alphanumeric`
     */
    #[inline]
    fn is_alphanumeric(&self) -> bool { is_alphanumeric(*self) }
    /// Returns the size in bytes not counting the null terminator
    #[inline]
    fn len(&self) -> uint { len(*self) }
    /// Returns the number of characters that a string holds
    #[inline]
    fn char_len(&self) -> uint { char_len(*self) }
    /**
     * Returns a slice of the given string from the byte range
     * [`begin`..`end`)
     *
     * Fails when `begin` and `end` do not point to valid characters or
     * beyond the last character of the string
     */
    #[inline]
    fn slice(&self, begin: uint, end: uint) -> &'self str {
        slice(*self, begin, end)
    }
    /// Splits a string into substrings using a character function
    #[inline]
    fn split(&self, sepfn: &fn(char) -> bool) -> ~[~str] {
        split(*self, sepfn)
    }
    /**
     * Splits a string into substrings at each occurrence of a given character
     */
    #[inline]
    fn split_char(&self, sep: char) -> ~[~str] { split_char(*self, sep) }
    /**
     * Splits a string into a vector of the substrings separated by a given
     * string
     */
    #[inline]
    fn split_str(&self, sep: &'a str) -> ~[~str] { split_str(*self, sep) }
    /// Returns true if one string starts with another
    #[inline]
    fn starts_with(&self, needle: &'a str) -> bool {
        starts_with(*self, needle)
    }
    /**
     * Take a substring of another.
     *
     * Returns a string containing `n` characters starting at byte offset
     * `begin`.
     */
    #[inline]
    fn substr(&self, begin: uint, n: uint) -> &'self str {
        substr(*self, begin, n)
    }
    /// Convert a string to lowercase
    #[inline]
    fn to_lower(&self) -> ~str { to_lower(*self) }
    /// Convert a string to uppercase
    #[inline]
    fn to_upper(&self) -> ~str { to_upper(*self) }
    /// Escape each char in `s` with char::escape_default.
    #[inline]
    fn escape_default(&self) -> ~str { escape_default(*self) }
    /// Escape each char in `s` with char::escape_unicode.
    #[inline]
    fn escape_unicode(&self) -> ~str { escape_unicode(*self) }

    /// Returns a string with leading and trailing whitespace removed
    #[inline]
    fn trim(&self) -> &'self str { trim(*self) }
    /// Returns a string with leading whitespace removed
    #[inline]
    fn trim_left(&self) -> &'self str { trim_left(*self) }
    /// Returns a string with trailing whitespace removed
    #[inline]
    fn trim_right(&self) -> &'self str { trim_right(*self) }

    #[inline]
    fn trim_chars(&self, chars_to_trim: &[char]) -> &'self str {
        trim_chars(*self, chars_to_trim)
    }
    #[inline]
    fn trim_left_chars(&self, chars_to_trim: &[char]) -> &'self str {
        trim_left_chars(*self, chars_to_trim)
    }
    #[inline]
    fn trim_right_chars(&self, chars_to_trim: &[char]) -> &'self str {
        trim_right_chars(*self, chars_to_trim)
    }


    #[inline]
    fn to_owned(&self) -> ~str { from_slice(*self) }

    #[inline]
    fn to_managed(&self) -> @str {
        let v = at_vec::from_fn(self.len() + 1, |i| {
            if i == self.len() { 0 } else { self[i] }
        });
        unsafe { ::cast::transmute(v) }
    }

    #[inline]
    fn char_at(&self, i: uint) -> char { char_at(*self, i) }

    #[inline]
    fn char_at_reverse(&self, i: uint) -> char {
        char_at_reverse(*self, i)
    }

    fn to_bytes(&self) -> ~[u8] { to_bytes(*self) }
}

pub trait OwnedStr {
    fn push_str(&mut self, v: &str);
    fn push_char(&mut self, c: char);
}

impl OwnedStr for ~str {
    fn push_str(&mut self, v: &str) {
        push_str(self, v);
    }

    fn push_char(&mut self, c: char) {
        push_char(self, c);
    }
}

impl Clone for ~str {
    #[inline(always)]
    fn clone(&self) -> ~str {
        self.to_str()  // hilarious
    }
}

#[cfg(test)]
mod tests {
    use char;
    use option::Some;
    use libc::c_char;
    use libc;
    use ptr;
    use str::*;
    use vec;
    use cmp::{TotalOrd, Less, Equal, Greater};

    #[test]
    fn test_eq() {
        fail_unless!((eq(&~"", &~"")));
        fail_unless!((eq(&~"foo", &~"foo")));
        fail_unless!((!eq(&~"foo", &~"bar")));
    }

    #[test]
    fn test_eq_slice() {
        fail_unless!((eq_slice(slice("foobar", 0, 3), "foo")));
        fail_unless!((eq_slice(slice("barfoo", 3, 6), "foo")));
        fail_unless!((!eq_slice("foo1", "foo2")));
    }

    #[test]
    fn test_le() {
        fail_unless!((le(&"", &"")));
        fail_unless!((le(&"", &"foo")));
        fail_unless!((le(&"foo", &"foo")));
        fail_unless!((!eq(&~"foo", &~"bar")));
    }

    #[test]
    fn test_len() {
        fail_unless!((len(~"") == 0u));
        fail_unless!((len(~"hello world") == 11u));
        fail_unless!((len(~"\x63") == 1u));
        fail_unless!((len(~"\xa2") == 2u));
        fail_unless!((len(~"\u03c0") == 2u));
        fail_unless!((len(~"\u2620") == 3u));
        fail_unless!((len(~"\U0001d11e") == 4u));

        fail_unless!((char_len(~"") == 0u));
        fail_unless!((char_len(~"hello world") == 11u));
        fail_unless!((char_len(~"\x63") == 1u));
        fail_unless!((char_len(~"\xa2") == 1u));
        fail_unless!((char_len(~"\u03c0") == 1u));
        fail_unless!((char_len(~"\u2620") == 1u));
        fail_unless!((char_len(~"\U0001d11e") == 1u));
        fail_unless!((char_len(~"ประเทศไทย中华Việt Nam") == 19u));
    }

    #[test]
    fn test_rfind_char() {
        fail_unless!(rfind_char(~"hello", 'l') == Some(3u));
        fail_unless!(rfind_char(~"hello", 'o') == Some(4u));
        fail_unless!(rfind_char(~"hello", 'h') == Some(0u));
        fail_unless!(rfind_char(~"hello", 'z').is_none());
        fail_unless!(rfind_char(~"ประเทศไทย中华Việt Nam", '华') == Some(30u));
    }

    #[test]
    fn test_pop_char() {
        let mut data = ~"ประเทศไทย中华";
        let cc = pop_char(&mut data);
        fail_unless!(~"ประเทศไทย中" == data);
        fail_unless!('华' == cc);
    }

    #[test]
    fn test_pop_char_2() {
        let mut data2 = ~"华";
        let cc2 = pop_char(&mut data2);
        fail_unless!(~"" == data2);
        fail_unless!('华' == cc2);
    }

    #[test]
    #[should_fail]
    #[ignore(cfg(windows))]
    fn test_pop_char_fail() {
        let mut data = ~"";
        let _cc3 = pop_char(&mut data);
    }

    #[test]
    fn test_split_char() {
        fn t(s: &str, c: char, u: &[~str]) {
            debug!(~"split_byte: " + s);
            let v = split_char(s, c);
            debug!("split_byte to: %?", v);
            fail_unless!(vec::all2(v, u, |a,b| a == b));
        }
        t(~"abc.hello.there", '.', ~[~"abc", ~"hello", ~"there"]);
        t(~".hello.there", '.', ~[~"", ~"hello", ~"there"]);
        t(~"...hello.there.", '.', ~[~"", ~"", ~"", ~"hello", ~"there", ~""]);

        fail_unless!(~[~"", ~"", ~"", ~"hello", ~"there", ~""]
                     == split_char(~"...hello.there.", '.'));

        fail_unless!(~[~""] == split_char(~"", 'z'));
        fail_unless!(~[~"",~""] == split_char(~"z", 'z'));
        fail_unless!(~[~"ok"] == split_char(~"ok", 'z'));
    }

    #[test]
    fn test_split_char_2() {
        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย中华", ~"iệt Nam"]
                     == split_char(data, 'V'));
        fail_unless!(~[~"ประเ", ~"ศไ", ~"ย中华Việt Nam"]
                     == split_char(data, 'ท'));
    }

    #[test]
    fn test_splitn_char() {
        fn t(s: &str, c: char, n: uint, u: &[~str]) {
            debug!(~"splitn_byte: " + s);
            let v = splitn_char(s, c, n);
            debug!("split_byte to: %?", v);
            debug!("comparing vs. %?", u);
            fail_unless!(vec::all2(v, u, |a,b| a == b));
        }
        t(~"abc.hello.there", '.', 0u, ~[~"abc.hello.there"]);
        t(~"abc.hello.there", '.', 1u, ~[~"abc", ~"hello.there"]);
        t(~"abc.hello.there", '.', 2u, ~[~"abc", ~"hello", ~"there"]);
        t(~"abc.hello.there", '.', 3u, ~[~"abc", ~"hello", ~"there"]);
        t(~".hello.there", '.', 0u, ~[~".hello.there"]);
        t(~".hello.there", '.', 1u, ~[~"", ~"hello.there"]);
        t(~"...hello.there.", '.', 3u, ~[~"", ~"", ~"", ~"hello.there."]);
        t(~"...hello.there.", '.', 5u,
          ~[~"", ~"", ~"", ~"hello", ~"there", ~""]);

        fail_unless!(~[~""] == splitn_char(~"", 'z', 5u));
        fail_unless!(~[~"",~""] == splitn_char(~"z", 'z', 5u));
        fail_unless!(~[~"ok"] == splitn_char(~"ok", 'z', 5u));
        fail_unless!(~[~"z"] == splitn_char(~"z", 'z', 0u));
        fail_unless!(~[~"w.x.y"] == splitn_char(~"w.x.y", '.', 0u));
        fail_unless!(~[~"w",~"x.y"] == splitn_char(~"w.x.y", '.', 1u));
    }

    #[test]
    fn test_splitn_char_2 () {
        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย中", ~"Việt Nam"]
                     == splitn_char(data, '华', 1u));

        fail_unless!(~[~"", ~"", ~"XXX", ~"YYYzWWWz"]
                     == splitn_char(~"zzXXXzYYYzWWWz", 'z', 3u));
        fail_unless!(~[~"",~""] == splitn_char(~"z", 'z', 5u));
        fail_unless!(~[~""] == splitn_char(~"", 'z', 5u));
        fail_unless!(~[~"ok"] == splitn_char(~"ok", 'z', 5u));
    }


    #[test]
    fn test_splitn_char_3() {
        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย中华", ~"iệt Nam"]
                     == splitn_char(data, 'V', 1u));
        fail_unless!(~[~"ประเ", ~"ศไทย中华Việt Nam"]
                     == splitn_char(data, 'ท', 1u));

    }

    #[test]
    fn test_split_char_no_trailing() {
     fn t(s: &str, c: char, u: &[~str]) {
            debug!(~"split_byte: " + s);
            let v = split_char_no_trailing(s, c);
            debug!("split_byte to: %?", v);
            fail_unless!(vec::all2(v, u, |a,b| a == b));
        }
        t(~"abc.hello.there", '.', ~[~"abc", ~"hello", ~"there"]);
        t(~".hello.there", '.', ~[~"", ~"hello", ~"there"]);
        t(~"...hello.there.", '.', ~[~"", ~"", ~"", ~"hello", ~"there"]);

        fail_unless!(~[~"", ~"", ~"", ~"hello", ~"there"]
                     == split_char_no_trailing(~"...hello.there.", '.'));

        fail_unless!(~[] == split_char_no_trailing(~"", 'z'));
        fail_unless!(~[~""] == split_char_no_trailing(~"z", 'z'));
        fail_unless!(~[~"ok"] == split_char_no_trailing(~"ok", 'z'));
    }

    #[test]
    fn test_split_char_no_trailing_2() {
        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย中华", ~"iệt Nam"]
                     == split_char_no_trailing(data, 'V'));
        fail_unless!(~[~"ประเ", ~"ศไ", ~"ย中华Việt Nam"]
                     == split_char_no_trailing(data, 'ท'));
    }

    #[test]
    fn test_split_str() {
        fn t(s: &str, sep: &'a str, i: int, k: &str) {
            fn borrow(x: &'a str) -> &'a str { x }
            let v = split_str(s, sep);
            fail_unless!(borrow(v[i]) == k);
        }

        t(~"--1233345--", ~"12345", 0, ~"--1233345--");
        t(~"abc::hello::there", ~"::", 0, ~"abc");
        t(~"abc::hello::there", ~"::", 1, ~"hello");
        t(~"abc::hello::there", ~"::", 2, ~"there");
        t(~"::hello::there", ~"::", 0, ~"");
        t(~"hello::there::", ~"::", 2, ~"");
        t(~"::hello::there::", ~"::", 3, ~"");

        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย", ~"Việt Nam"]
                     == split_str (data, ~"中华"));

        fail_unless!(~[~"", ~"XXX", ~"YYY", ~""]
                     == split_str(~"zzXXXzzYYYzz", ~"zz"));

        fail_unless!(~[~"zz", ~"zYYYz"] == split_str(~"zzXXXzYYYz", ~"XXX"));


        fail_unless!(~[~"", ~"XXX", ~"YYY", ~""] ==
                     split_str(~".XXX.YYY.", ~"."));
        fail_unless!(~[~""] == split_str(~"", ~"."));
        fail_unless!(~[~"",~""] == split_str(~"zz", ~"zz"));
        fail_unless!(~[~"ok"] == split_str(~"ok", ~"z"));
        fail_unless!(~[~"",~"z"] == split_str(~"zzz", ~"zz"));
        fail_unless!(~[~"",~"",~"z"] == split_str(~"zzzzz", ~"zz"));
    }


    #[test]
    fn test_split() {
        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย中", ~"Việt Nam"]
                     == split (data, |cc| cc == '华'));

        fail_unless!(~[~"", ~"", ~"XXX", ~"YYY", ~""]
                     == split(~"zzXXXzYYYz", char::is_lowercase));

        fail_unless!(~[~"zz", ~"", ~"", ~"z", ~"", ~"", ~"z"]
                     == split(~"zzXXXzYYYz", char::is_uppercase));

        fail_unless!(~[~"",~""] == split(~"z", |cc| cc == 'z'));
        fail_unless!(~[~""] == split(~"", |cc| cc == 'z'));
        fail_unless!(~[~"ok"] == split(~"ok", |cc| cc == 'z'));
    }

    #[test]
    fn test_split_no_trailing() {
        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(~[~"ประเทศไทย中", ~"Việt Nam"]
                     == split_no_trailing (data, |cc| cc == '华'));

        fail_unless!(~[~"", ~"", ~"XXX", ~"YYY"]
                     == split_no_trailing(~"zzXXXzYYYz", char::is_lowercase));

        fail_unless!(~[~"zz", ~"", ~"", ~"z", ~"", ~"", ~"z"]
                     == split_no_trailing(~"zzXXXzYYYz", char::is_uppercase));

        fail_unless!(~[~""] == split_no_trailing(~"z", |cc| cc == 'z'));
        fail_unless!(~[] == split_no_trailing(~"", |cc| cc == 'z'));
        fail_unless!(~[~"ok"] == split_no_trailing(~"ok", |cc| cc == 'z'));
    }

    #[test]
    fn test_lines() {
        let lf = ~"\nMary had a little lamb\nLittle lamb\n";
        let crlf = ~"\r\nMary had a little lamb\r\nLittle lamb\r\n";

        fail_unless!(~[~"", ~"Mary had a little lamb", ~"Little lamb"]
                     == lines(lf));

        fail_unless!(~[~"", ~"Mary had a little lamb", ~"Little lamb"]
                     == lines_any(lf));

        fail_unless!(~[~"\r", ~"Mary had a little lamb\r",
                       ~"Little lamb\r"]
            == lines(crlf));

        fail_unless!(~[~"", ~"Mary had a little lamb", ~"Little lamb"]
            == lines_any(crlf));

        fail_unless!(~[] == lines    (~""));
        fail_unless!(~[] == lines_any(~""));
        fail_unless!(~[~""] == lines    (~"\n"));
        fail_unless!(~[~""] == lines_any(~"\n"));
        fail_unless!(~[~"banana"] == lines    (~"banana"));
        fail_unless!(~[~"banana"] == lines_any(~"banana"));
    }

    #[test]
    fn test_words () {
        let data = ~"\nMary had a little lamb\nLittle lamb\n";
        fail_unless!(~[
            ~"Mary",~"had",~"a",~"little",~"lamb",~"Little",~"lamb"]
            == words(data));

        fail_unless!(~[~"ok"] == words(~"ok"));
        fail_unless!(~[] == words(~""));
    }

    #[test]
    fn test_split_within() {
        fail_unless!(split_within(~"", 0) == ~[]);
        fail_unless!(split_within(~"", 15) == ~[]);
        fail_unless!(split_within(~"hello", 15) == ~[~"hello"]);

        let data = ~"\nMary had a little lamb\nLittle lamb\n";
        error!("~~~~ %?", split_within(data, 15));
        fail_unless!(split_within(data, 15) == ~[~"Mary had a",
                                                 ~"little lamb",
                                                 ~"Little lamb"]);
    }

    #[test]
    fn test_find_str() {
        // byte positions
        fail_unless!(find_str(~"banana", ~"apple pie").is_none());
        fail_unless!(find_str(~"", ~"") == Some(0u));

        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(find_str(data, ~"")     == Some(0u));
        fail_unless!(find_str(data, ~"ประเ") == Some( 0u));
        fail_unless!(find_str(data, ~"ะเ")   == Some( 6u));
        fail_unless!(find_str(data, ~"中华") == Some(27u));
        fail_unless!(find_str(data, ~"ไท华").is_none());
    }

    #[test]
    fn test_find_str_between() {
        // byte positions
        fail_unless!(find_str_between(~"", ~"", 0u, 0u) == Some(0u));

        let data = ~"abcabc";
        fail_unless!(find_str_between(data, ~"ab", 0u, 6u) == Some(0u));
        fail_unless!(find_str_between(data, ~"ab", 2u, 6u) == Some(3u));
        fail_unless!(find_str_between(data, ~"ab", 2u, 4u).is_none());

        let mut data = ~"ประเทศไทย中华Việt Nam";
        data = data + data;
        fail_unless!(find_str_between(data, ~"", 0u, 43u) == Some(0u));
        fail_unless!(find_str_between(data, ~"", 6u, 43u) == Some(6u));

        fail_unless!(find_str_between(data, ~"ประ", 0u, 43u) == Some( 0u));
        fail_unless!(find_str_between(data, ~"ทศไ", 0u, 43u) == Some(12u));
        fail_unless!(find_str_between(data, ~"ย中", 0u, 43u) == Some(24u));
        fail_unless!(find_str_between(data, ~"iệt", 0u, 43u) == Some(34u));
        fail_unless!(find_str_between(data, ~"Nam", 0u, 43u) == Some(40u));

        fail_unless!(find_str_between(data, ~"ประ", 43u, 86u) == Some(43u));
        fail_unless!(find_str_between(data, ~"ทศไ", 43u, 86u) == Some(55u));
        fail_unless!(find_str_between(data, ~"ย中", 43u, 86u) == Some(67u));
        fail_unless!(find_str_between(data, ~"iệt", 43u, 86u) == Some(77u));
        fail_unless!(find_str_between(data, ~"Nam", 43u, 86u) == Some(83u));
    }

    #[test]
    fn test_substr() {
        fn t(a: &str, b: &str, start: int) {
            fail_unless!(substr(a, start as uint, len(b)) == b);
        }
        t("hello", "llo", 2);
        t("hello", "el", 1);
        fail_unless!("ะเทศไท" == substr("ประเทศไทย中华Việt Nam", 6u, 6u));
    }

    #[test]
    fn test_concat() {
        fn t(v: &[~str], s: &str) {
            fail_unless!(concat(v) == s.to_str());
        }
        t(~[~"you", ~"know", ~"I'm", ~"no", ~"good"], ~"youknowI'mnogood");
        let v: ~[~str] = ~[];
        t(v, ~"");
        t(~[~"hi"], ~"hi");
    }

    #[test]
    fn test_connect() {
        fn t(v: &[~str], sep: &str, s: &str) {
            fail_unless!(connect(v, sep) == s.to_str());
        }
        t(~[~"you", ~"know", ~"I'm", ~"no", ~"good"],
          ~" ", ~"you know I'm no good");
        let v: ~[~str] = ~[];
        t(v, ~" ", ~"");
        t(~[~"hi"], ~" ", ~"hi");
    }

    #[test]
    fn test_connect_slices() {
        fn t(v: &[&str], sep: &str, s: &str) {
            fail_unless!(connect_slices(v, sep) == s.to_str());
        }
        t(["you", "know", "I'm", "no", "good"],
          " ", "you know I'm no good");
        t([], " ", "");
        t(["hi"], " ", "hi");
    }

    #[test]
    fn test_repeat() {
        fail_unless!(repeat(~"x", 4) == ~"xxxx");
        fail_unless!(repeat(~"hi", 4) == ~"hihihihi");
        fail_unless!(repeat(~"ไท华", 3) == ~"ไท华ไท华ไท华");
        fail_unless!(repeat(~"", 4) == ~"");
        fail_unless!(repeat(~"hi", 0) == ~"");
    }

    #[test]
    fn test_to_upper() {
        // libc::toupper, and hence str::to_upper
        // are culturally insensitive: they only work for ASCII
        // (see Issue #1347)
        let unicode = ~""; //"\u65e5\u672c"; // uncomment once non-ASCII works
        let input = ~"abcDEF" + unicode + ~"xyz:.;";
        let expected = ~"ABCDEF" + unicode + ~"XYZ:.;";
        let actual = to_upper(input);
        fail_unless!(expected == actual);
    }

    #[test]
    fn test_to_lower() {
        unsafe {
            fail_unless!(~"" == map(~"",
                |c| libc::tolower(c as c_char) as char));
            fail_unless!(~"ymca" == map(~"YMCA",
                |c| libc::tolower(c as c_char) as char));
        }
    }

    #[test]
    fn test_unsafe_slice() {
        fail_unless!("ab" == unsafe {raw::slice_bytes("abc", 0, 2)});
        fail_unless!("bc" == unsafe {raw::slice_bytes("abc", 1, 3)});
        fail_unless!("" == unsafe {raw::slice_bytes("abc", 1, 1)});
        fn a_million_letter_a() -> ~str {
            let mut i = 0;
            let mut rs = ~"";
            while i < 100000 { push_str(&mut rs, "aaaaaaaaaa"); i += 1; }
            rs
        }
        fn half_a_million_letter_a() -> ~str {
            let mut i = 0;
            let mut rs = ~"";
            while i < 100000 { push_str(&mut rs, "aaaaa"); i += 1; }
            rs
        }
        let letters = a_million_letter_a();
        fail_unless!(half_a_million_letter_a() ==
            unsafe {raw::slice_bytes(letters, 0u, 500000)}.to_owned());
    }

    #[test]
    fn test_starts_with() {
        fail_unless!((starts_with(~"", ~"")));
        fail_unless!((starts_with(~"abc", ~"")));
        fail_unless!((starts_with(~"abc", ~"a")));
        fail_unless!((!starts_with(~"a", ~"abc")));
        fail_unless!((!starts_with(~"", ~"abc")));
    }

    #[test]
    fn test_ends_with() {
        fail_unless!((ends_with(~"", ~"")));
        fail_unless!((ends_with(~"abc", ~"")));
        fail_unless!((ends_with(~"abc", ~"c")));
        fail_unless!((!ends_with(~"a", ~"abc")));
        fail_unless!((!ends_with(~"", ~"abc")));
    }

    #[test]
    fn test_is_empty() {
        fail_unless!((is_empty(~"")));
        fail_unless!((!is_empty(~"a")));
    }

    #[test]
    fn test_replace() {
        let a = ~"a";
        fail_unless!(replace(~"", a, ~"b") == ~"");
        fail_unless!(replace(~"a", a, ~"b") == ~"b");
        fail_unless!(replace(~"ab", a, ~"b") == ~"bb");
        let test = ~"test";
        fail_unless!(replace(~" test test ", test, ~"toast") ==
            ~" toast toast ");
        fail_unless!(replace(~" test test ", test, ~"") == ~"   ");
    }

    #[test]
    fn test_replace_2a() {
        let data = ~"ประเทศไทย中华";
        let repl = ~"دولة الكويت";

        let a = ~"ประเ";
        let A = ~"دولة الكويتทศไทย中华";
        fail_unless!((replace(data, a, repl) ==  A));
    }

    #[test]
    fn test_replace_2b() {
        let data = ~"ประเทศไทย中华";
        let repl = ~"دولة الكويت";

        let b = ~"ะเ";
        let B = ~"ปรدولة الكويتทศไทย中华";
        fail_unless!((replace(data, b,   repl) ==  B));
    }

    #[test]
    fn test_replace_2c() {
        let data = ~"ประเทศไทย中华";
        let repl = ~"دولة الكويت";

        let c = ~"中华";
        let C = ~"ประเทศไทยدولة الكويت";
        fail_unless!((replace(data, c, repl) ==  C));
    }

    #[test]
    fn test_replace_2d() {
        let data = ~"ประเทศไทย中华";
        let repl = ~"دولة الكويت";

        let d = ~"ไท华";
        fail_unless!((replace(data, d, repl) == data));
    }

    #[test]
    fn test_slice() {
        fail_unless!("ab" == slice("abc", 0, 2));
        fail_unless!("bc" == slice("abc", 1, 3));
        fail_unless!("" == slice("abc", 1, 1));
        fail_unless!("\u65e5" == slice("\u65e5\u672c", 0, 3));

        let data = "ประเทศไทย中华";
        fail_unless!("ป" == slice(data, 0, 3));
        fail_unless!("ร" == slice(data, 3, 6));
        fail_unless!("" == slice(data, 3, 3));
        fail_unless!("华" == slice(data, 30, 33));

        fn a_million_letter_X() -> ~str {
            let mut i = 0;
            let mut rs = ~"";
            while i < 100000 {
                push_str(&mut rs, "华华华华华华华华华华");
                i += 1;
            }
            rs
        }
        fn half_a_million_letter_X() -> ~str {
            let mut i = 0;
            let mut rs = ~"";
            while i < 100000 { push_str(&mut rs, "华华华华华"); i += 1; }
            rs
        }
        let letters = a_million_letter_X();
        fail_unless!(half_a_million_letter_X() ==
            slice(letters, 0u, 3u * 500000u).to_owned());
    }

    #[test]
    fn test_slice_2() {
        let ss = "中华Việt Nam";

        fail_unless!("华" == slice(ss, 3u, 6u));
        fail_unless!("Việt Nam" == slice(ss, 6u, 16u));

        fail_unless!("ab" == slice("abc", 0u, 2u));
        fail_unless!("bc" == slice("abc", 1u, 3u));
        fail_unless!("" == slice("abc", 1u, 1u));

        fail_unless!("中" == slice(ss, 0u, 3u));
        fail_unless!("华V" == slice(ss, 3u, 7u));
        fail_unless!("" == slice(ss, 3u, 3u));
        /*0: 中
          3: 华
          6: V
          7: i
          8: ệ
         11: t
         12:
         13: N
         14: a
         15: m */
    }

    #[test]
    #[should_fail]
    #[ignore(cfg(windows))]
    fn test_slice_fail() {
        slice("中华Việt Nam", 0u, 2u);
    }

    #[test]
    fn test_trim_left_chars() {
        fail_unless!(trim_left_chars(" *** foo *** ", ~[]) ==
                     " *** foo *** ");
        fail_unless!(trim_left_chars(" *** foo *** ", ~['*', ' ']) ==
                     "foo *** ");
        fail_unless!(trim_left_chars(" ***  *** ", ~['*', ' ']) == "");
        fail_unless!(trim_left_chars("foo *** ", ~['*', ' ']) ==
                     "foo *** ");
    }

    #[test]
    fn test_trim_right_chars() {
        fail_unless!(trim_right_chars(" *** foo *** ", ~[]) ==
                     " *** foo *** ");
        fail_unless!(trim_right_chars(" *** foo *** ", ~['*', ' ']) ==
                     " *** foo");
        fail_unless!(trim_right_chars(" ***  *** ", ~['*', ' ']) == "");
        fail_unless!(trim_right_chars(" *** foo", ~['*', ' ']) ==
                     " *** foo");
    }

    #[test]
    fn test_trim_chars() {
        fail_unless!(trim_chars(" *** foo *** ", ~[]) == " *** foo *** ");
        fail_unless!(trim_chars(" *** foo *** ", ~['*', ' ']) == "foo");
        fail_unless!(trim_chars(" ***  *** ", ~['*', ' ']) == "");
        fail_unless!(trim_chars("foo", ~['*', ' ']) == "foo");
    }

    #[test]
    fn test_trim_left() {
        fail_unless!((trim_left("") == ""));
        fail_unless!((trim_left("a") == "a"));
        fail_unless!((trim_left("    ") == ""));
        fail_unless!((trim_left("     blah") == "blah"));
        fail_unless!((trim_left("   \u3000  wut") == "wut"));
        fail_unless!((trim_left("hey ") == "hey "));
    }

    #[test]
    fn test_trim_right() {
        fail_unless!((trim_right("") == ""));
        fail_unless!((trim_right("a") == "a"));
        fail_unless!((trim_right("    ") == ""));
        fail_unless!((trim_right("blah     ") == "blah"));
        fail_unless!((trim_right("wut   \u3000  ") == "wut"));
        fail_unless!((trim_right(" hey") == " hey"));
    }

    #[test]
    fn test_trim() {
        fail_unless!((trim("") == ""));
        fail_unless!((trim("a") == "a"));
        fail_unless!((trim("    ") == ""));
        fail_unless!((trim("    blah     ") == "blah"));
        fail_unless!((trim("\nwut   \u3000  ") == "wut"));
        fail_unless!((trim(" hey dude ") == "hey dude"));
    }

    #[test]
    fn test_is_whitespace() {
        fail_unless!((is_whitespace(~"")));
        fail_unless!((is_whitespace(~" ")));
        fail_unless!((is_whitespace(~"\u2009"))); // Thin space
        fail_unless!((is_whitespace(~"  \n\t   ")));
        fail_unless!((!is_whitespace(~"   _   ")));
    }

    #[test]
    fn test_is_ascii() {
        fail_unless!((is_ascii(~"")));
        fail_unless!((is_ascii(~"a")));
        fail_unless!((!is_ascii(~"\u2009")));
    }

    #[test]
    fn test_shift_byte() {
        let mut s = ~"ABC";
        let b = unsafe { raw::shift_byte(&mut s) };
        fail_unless!((s == ~"BC"));
        fail_unless!((b == 65u8));
    }

    #[test]
    fn test_pop_byte() {
        let mut s = ~"ABC";
        let b = unsafe { raw::pop_byte(&mut s) };
        fail_unless!((s == ~"AB"));
        fail_unless!((b == 67u8));
    }

    #[test]
    fn test_unsafe_from_bytes() {
        let a = ~[65u8, 65u8, 65u8, 65u8, 65u8, 65u8, 65u8];
        let b = unsafe { raw::from_bytes(a) };
        fail_unless!((b == ~"AAAAAAA"));
    }

    #[test]
    fn test_from_bytes() {
        let ss = ~"ศไทย中华Việt Nam";
        let bb = ~[0xe0_u8, 0xb8_u8, 0xa8_u8,
                  0xe0_u8, 0xb9_u8, 0x84_u8,
                  0xe0_u8, 0xb8_u8, 0x97_u8,
                  0xe0_u8, 0xb8_u8, 0xa2_u8,
                  0xe4_u8, 0xb8_u8, 0xad_u8,
                  0xe5_u8, 0x8d_u8, 0x8e_u8,
                  0x56_u8, 0x69_u8, 0xe1_u8,
                  0xbb_u8, 0x87_u8, 0x74_u8,
                  0x20_u8, 0x4e_u8, 0x61_u8,
                  0x6d_u8];

        fail_unless!(ss == from_bytes(bb));
    }

    #[test]
    #[should_fail]
    #[ignore(cfg(windows))]
    fn test_from_bytes_fail() {
        let bb = ~[0xff_u8, 0xb8_u8, 0xa8_u8,
                  0xe0_u8, 0xb9_u8, 0x84_u8,
                  0xe0_u8, 0xb8_u8, 0x97_u8,
                  0xe0_u8, 0xb8_u8, 0xa2_u8,
                  0xe4_u8, 0xb8_u8, 0xad_u8,
                  0xe5_u8, 0x8d_u8, 0x8e_u8,
                  0x56_u8, 0x69_u8, 0xe1_u8,
                  0xbb_u8, 0x87_u8, 0x74_u8,
                  0x20_u8, 0x4e_u8, 0x61_u8,
                  0x6d_u8];

         let _x = from_bytes(bb);
    }

    #[test]
    fn test_from_buf() {
        unsafe {
            let a = ~[65u8, 65u8, 65u8, 65u8, 65u8, 65u8, 65u8, 0u8];
            let b = vec::raw::to_ptr(a);
            let c = raw::from_buf(b);
            fail_unless!((c == ~"AAAAAAA"));
        }
    }

    #[test]
    #[ignore(cfg(windows))]
    #[should_fail]
    fn test_as_bytes_fail() {
        // Don't double free
        as_bytes::<()>(&~"", |_bytes| fail!() );
    }

    #[test]
    fn test_as_buf() {
        let a = ~"Abcdefg";
        let b = as_buf(a, |buf, _l| {
            fail_unless!(unsafe { *buf } == 65u8);
            100
        });
        fail_unless!((b == 100));
    }

    #[test]
    fn test_as_buf_small() {
        let a = ~"A";
        let b = as_buf(a, |buf, _l| {
            fail_unless!(unsafe { *buf } == 65u8);
            100
        });
        fail_unless!((b == 100));
    }

    #[test]
    fn test_as_buf2() {
        unsafe {
            let s = ~"hello";
            let sb = as_buf(s, |b, _l| b);
            let s_cstr = raw::from_buf(sb);
            fail_unless!(s_cstr == s);
        }
    }

    #[test]
    fn test_as_buf_3() {
        let a = ~"hello";
        do as_buf(a) |buf, len| {
            unsafe {
                fail_unless!(a[0] == 'h' as u8);
                fail_unless!(*buf == 'h' as u8);
                fail_unless!(len == 6u);
                fail_unless!(*ptr::offset(buf,4u) == 'o' as u8);
                fail_unless!(*ptr::offset(buf,5u) == 0u8);
            }
        }
    }

    #[test]
    fn vec_str_conversions() {
        let s1: ~str = ~"All mimsy were the borogoves";

        let v: ~[u8] = to_bytes(s1);
        let s2: ~str = from_bytes(v);
        let mut i: uint = 0u;
        let n1: uint = len(s1);
        let n2: uint = vec::len::<u8>(v);
        fail_unless!((n1 == n2));
        while i < n1 {
            let a: u8 = s1[i];
            let b: u8 = s2[i];
            debug!(a);
            debug!(b);
            fail_unless!((a == b));
            i += 1u;
        }
    }

    #[test]
    fn test_contains() {
        fail_unless!(contains(~"abcde", ~"bcd"));
        fail_unless!(contains(~"abcde", ~"abcd"));
        fail_unless!(contains(~"abcde", ~"bcde"));
        fail_unless!(contains(~"abcde", ~""));
        fail_unless!(contains(~"", ~""));
        fail_unless!(!contains(~"abcde", ~"def"));
        fail_unless!(!contains(~"", ~"a"));

        let data = ~"ประเทศไทย中华Việt Nam";
        fail_unless!(contains(data, ~"ประเ"));
        fail_unless!(contains(data, ~"ะเ"));
        fail_unless!(contains(data, ~"中华"));
        fail_unless!(!contains(data, ~"ไท华"));
    }

    #[test]
    fn test_contains_char() {
        fail_unless!(contains_char(~"abc", 'b'));
        fail_unless!(contains_char(~"a", 'a'));
        fail_unless!(!contains_char(~"abc", 'd'));
        fail_unless!(!contains_char(~"", 'a'));
    }

    #[test]
    fn test_split_char_each() {
        let data = ~"\nMary had a little lamb\nLittle lamb\n";

        let mut ii = 0;

        for split_char_each(data, ' ') |xx| {
            match ii {
              0 => fail_unless!("\nMary" == xx),
              1 => fail_unless!("had"    == xx),
              2 => fail_unless!("a"      == xx),
              3 => fail_unless!("little" == xx),
              _ => ()
            }
            ii += 1;
        }
    }

    #[test]
    fn test_splitn_char_each() {
        let data = ~"\nMary had a little lamb\nLittle lamb\n";

        let mut ii = 0;

        for splitn_char_each(data, ' ', 2u) |xx| {
            match ii {
              0 => fail_unless!("\nMary" == xx),
              1 => fail_unless!("had"    == xx),
              2 => fail_unless!("a little lamb\nLittle lamb\n" == xx),
              _ => ()
            }
            ii += 1;
        }
    }

    #[test]
    fn test_words_each() {
        let data = ~"\nMary had a little lamb\nLittle lamb\n";

        let mut ii = 0;

        for words_each(data) |ww| {
            match ii {
              0 => fail_unless!("Mary"   == ww),
              1 => fail_unless!("had"    == ww),
              2 => fail_unless!("a"      == ww),
              3 => fail_unless!("little" == ww),
              _ => ()
            }
            ii += 1;
        }

        words_each(~"", |_x| fail!()); // should not fail
    }

    #[test]
    fn test_lines_each () {
        let lf = ~"\nMary had a little lamb\nLittle lamb\n";

        let mut ii = 0;

        for lines_each(lf) |x| {
            match ii {
                0 => fail_unless!("" == x),
                1 => fail_unless!("Mary had a little lamb" == x),
                2 => fail_unless!("Little lamb" == x),
                _ => ()
            }
            ii += 1;
        }
    }

    #[test]
    fn test_map() {
        unsafe {
            fail_unless!(~"" == map(~"", |c|
                libc::toupper(c as c_char) as char));
            fail_unless!(~"YMCA" == map(~"ymca",
                                  |c| libc::toupper(c as c_char) as char));
        }
    }

    #[test]
    fn test_all() {
        fail_unless!(true  == all(~"", char::is_uppercase));
        fail_unless!(false == all(~"ymca", char::is_uppercase));
        fail_unless!(true  == all(~"YMCA", char::is_uppercase));
        fail_unless!(false == all(~"yMCA", char::is_uppercase));
        fail_unless!(false == all(~"YMCy", char::is_uppercase));
    }

    #[test]
    fn test_any() {
        fail_unless!(false  == any(~"", char::is_uppercase));
        fail_unless!(false == any(~"ymca", char::is_uppercase));
        fail_unless!(true  == any(~"YMCA", char::is_uppercase));
        fail_unless!(true == any(~"yMCA", char::is_uppercase));
        fail_unless!(true == any(~"Ymcy", char::is_uppercase));
    }

    #[test]
    fn test_chars() {
        let ss = ~"ศไทย中华Việt Nam";
        fail_unless!(~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a',
                       'm']
            == chars(ss));
    }

    #[test]
    fn test_utf16() {
        let pairs =
            ~[(~"𐍅𐌿𐌻𐍆𐌹𐌻𐌰\n",
              ~[0xd800_u16, 0xdf45_u16, 0xd800_u16, 0xdf3f_u16,
               0xd800_u16, 0xdf3b_u16, 0xd800_u16, 0xdf46_u16,
               0xd800_u16, 0xdf39_u16, 0xd800_u16, 0xdf3b_u16,
               0xd800_u16, 0xdf30_u16, 0x000a_u16]),

             (~"𐐒𐑉𐐮𐑀𐐲𐑋 𐐏𐐲𐑍\n",
              ~[0xd801_u16, 0xdc12_u16, 0xd801_u16,
               0xdc49_u16, 0xd801_u16, 0xdc2e_u16, 0xd801_u16,
               0xdc40_u16, 0xd801_u16, 0xdc32_u16, 0xd801_u16,
               0xdc4b_u16, 0x0020_u16, 0xd801_u16, 0xdc0f_u16,
               0xd801_u16, 0xdc32_u16, 0xd801_u16, 0xdc4d_u16,
               0x000a_u16]),

             (~"𐌀𐌖𐌋𐌄𐌑𐌉·𐌌𐌄𐌕𐌄𐌋𐌉𐌑\n",
              ~[0xd800_u16, 0xdf00_u16, 0xd800_u16, 0xdf16_u16,
               0xd800_u16, 0xdf0b_u16, 0xd800_u16, 0xdf04_u16,
               0xd800_u16, 0xdf11_u16, 0xd800_u16, 0xdf09_u16,
               0x00b7_u16, 0xd800_u16, 0xdf0c_u16, 0xd800_u16,
               0xdf04_u16, 0xd800_u16, 0xdf15_u16, 0xd800_u16,
               0xdf04_u16, 0xd800_u16, 0xdf0b_u16, 0xd800_u16,
               0xdf09_u16, 0xd800_u16, 0xdf11_u16, 0x000a_u16 ]),

             (~"𐒋𐒘𐒈𐒑𐒛𐒒 𐒕𐒓 𐒈𐒚𐒍 𐒏𐒜𐒒𐒖𐒆 𐒕𐒆\n",
              ~[0xd801_u16, 0xdc8b_u16, 0xd801_u16, 0xdc98_u16,
               0xd801_u16, 0xdc88_u16, 0xd801_u16, 0xdc91_u16,
               0xd801_u16, 0xdc9b_u16, 0xd801_u16, 0xdc92_u16,
               0x0020_u16, 0xd801_u16, 0xdc95_u16, 0xd801_u16,
               0xdc93_u16, 0x0020_u16, 0xd801_u16, 0xdc88_u16,
               0xd801_u16, 0xdc9a_u16, 0xd801_u16, 0xdc8d_u16,
               0x0020_u16, 0xd801_u16, 0xdc8f_u16, 0xd801_u16,
               0xdc9c_u16, 0xd801_u16, 0xdc92_u16, 0xd801_u16,
               0xdc96_u16, 0xd801_u16, 0xdc86_u16, 0x0020_u16,
               0xd801_u16, 0xdc95_u16, 0xd801_u16, 0xdc86_u16,
               0x000a_u16 ]) ];

        for vec::each(pairs) |p| {
            let (s, u) = copy *p;
            fail_unless!(to_utf16(s) == u);
            fail_unless!(from_utf16(u) == s);
            fail_unless!(from_utf16(to_utf16(s)) == s);
            fail_unless!(to_utf16(from_utf16(u)) == u);
        }
    }

    #[test]
    fn test_char_at() {
        let s = ~"ศไทย中华Việt Nam";
        let v = ~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a','m'];
        let mut pos = 0;
        for v.each |ch| {
            fail_unless!(s.char_at(pos) == *ch);
            pos += from_char(*ch).len();
        }
    }

    #[test]
    fn test_char_at_reverse() {
        let s = ~"ศไทย中华Việt Nam";
        let v = ~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a','m'];
        let mut pos = s.len();
        for v.each_reverse |ch| {
            fail_unless!(s.char_at_reverse(pos) == *ch);
            pos -= from_char(*ch).len();
        }
    }

    #[test]
    fn test_each() {
        let s = ~"ศไทย中华Việt Nam";
        let v = [
            224, 184, 168, 224, 185, 132, 224, 184, 151, 224, 184, 162, 228,
            184, 173, 229, 141, 142, 86, 105, 225, 187, 135, 116, 32, 78, 97,
            109
        ];
        let mut pos = 0;

        for s.each |b| {
            fail_unless!(b == v[pos]);
            pos += 1;
        }
    }

    #[test]
    fn test_each_empty() {
        for "".each |b| {
            fail_unless!(b == 0u8);
        }
    }

    #[test]
    fn test_eachi() {
        let s = ~"ศไทย中华Việt Nam";
        let v = [
            224, 184, 168, 224, 185, 132, 224, 184, 151, 224, 184, 162, 228,
            184, 173, 229, 141, 142, 86, 105, 225, 187, 135, 116, 32, 78, 97,
            109
        ];
        let mut pos = 0;

        for s.eachi |i, b| {
            fail_unless!(pos == i);
            fail_unless!(b == v[pos]);
            pos += 1;
        }
    }

    #[test]
    fn test_eachi_empty() {
        for "".eachi |i, b| {
            fail_unless!(i == 0);
            fail_unless!(b == 0);
        }
    }

    #[test]
    fn test_each_reverse() {
        let s = ~"ศไทย中华Việt Nam";
        let v = [
            224, 184, 168, 224, 185, 132, 224, 184, 151, 224, 184, 162, 228,
            184, 173, 229, 141, 142, 86, 105, 225, 187, 135, 116, 32, 78, 97,
            109
        ];
        let mut pos = v.len();

        for s.each_reverse |b| {
            pos -= 1;
            fail_unless!(b == v[pos]);
        }
    }

    #[test]
    fn test_each_empty_reverse() {
        for "".each_reverse |b| {
            fail_unless!(b == 0u8);
        }
    }

    #[test]
    fn test_eachi_reverse() {
        let s = ~"ศไทย中华Việt Nam";
        let v = [
            224, 184, 168, 224, 185, 132, 224, 184, 151, 224, 184, 162, 228,
            184, 173, 229, 141, 142, 86, 105, 225, 187, 135, 116, 32, 78, 97,
            109
        ];
        let mut pos = v.len();

        for s.eachi_reverse |i, b| {
            pos -= 1;
            fail_unless!(pos == i);
            fail_unless!(b == v[pos]);
        }
    }

    #[test]
    fn test_eachi_reverse_empty() {
        for "".eachi_reverse |i, b| {
            fail_unless!(i == 0);
            fail_unless!(b == 0);
        }
    }

    #[test]
    fn test_each_char() {
        let s = ~"ศไทย中华Việt Nam";
        let v = ~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a','m'];
        let mut pos = 0;
        for s.each_char |ch| {
            fail_unless!(ch == v[pos]);
            pos += 1;
        }
    }

    #[test]
    fn test_each_chari() {
        let s = ~"ศไทย中华Việt Nam";
        let v = ~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a','m'];
        let mut pos = 0;
        for s.each_chari |i, ch| {
            fail_unless!(pos == i);
            fail_unless!(ch == v[pos]);
            pos += 1;
        }
    }

    #[test]
    fn test_each_char_reverse() {
        let s = ~"ศไทย中华Việt Nam";
        let v = ~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a','m'];
        let mut pos = 0;
        for s.each_char_reverse |ch| {
            fail_unless!(ch == v[pos]);
            pos += 1;
        }
    }

    #[test]
    fn test_each_chari_reverse() {
        let s = ~"ศไทย中华Việt Nam";
        let v = ~['ศ','ไ','ท','ย','中','华','V','i','ệ','t',' ','N','a','m'];
        let mut pos = v.len();
        for s.each_chari_reverse |i, ch| {
            pos -= 1;
            fail_unless!(pos == i);
            fail_unless!(ch == v[pos]);
        }
    }

    #[test]
    fn test_escape_unicode() {
        fail_unless!(escape_unicode(~"abc") == ~"\\x61\\x62\\x63");
        fail_unless!(escape_unicode(~"a c") == ~"\\x61\\x20\\x63");
        fail_unless!(escape_unicode(~"\r\n\t") == ~"\\x0d\\x0a\\x09");
        fail_unless!(escape_unicode(~"'\"\\") == ~"\\x27\\x22\\x5c");
        fail_unless!(escape_unicode(~"\x00\x01\xfe\xff") ==
                     ~"\\x00\\x01\\xfe\\xff");
        fail_unless!(escape_unicode(~"\u0100\uffff") == ~"\\u0100\\uffff");
        fail_unless!(escape_unicode(~"\U00010000\U0010ffff") ==
            ~"\\U00010000\\U0010ffff");
        fail_unless!(escape_unicode(~"ab\ufb00") == ~"\\x61\\x62\\ufb00");
        fail_unless!(escape_unicode(~"\U0001d4ea\r") == ~"\\U0001d4ea\\x0d");
    }

    #[test]
    fn test_escape_default() {
        fail_unless!(escape_default(~"abc") == ~"abc");
        fail_unless!(escape_default(~"a c") == ~"a c");
        fail_unless!(escape_default(~"\r\n\t") == ~"\\r\\n\\t");
        fail_unless!(escape_default(~"'\"\\") == ~"\\'\\\"\\\\");
        fail_unless!(escape_default(~"\u0100\uffff") == ~"\\u0100\\uffff");
        fail_unless!(escape_default(~"\U00010000\U0010ffff") ==
            ~"\\U00010000\\U0010ffff");
        fail_unless!(escape_default(~"ab\ufb00") == ~"ab\\ufb00");
        fail_unless!(escape_default(~"\U0001d4ea\r") == ~"\\U0001d4ea\\r");
    }

    #[test]
    fn test_to_managed() {
        fail_unless!((~"abc").to_managed() == @"abc");
        fail_unless!(slice("abcdef", 1, 5).to_managed() == @"bcde");
    }

    #[test]
    fn test_total_ord() {
        "1234".cmp(& &"123") == Greater;
        "123".cmp(& &"1234") == Less;
        "1234".cmp(& &"1234") == Equal;
        "12345555".cmp(& &"123456") == Less;
        "22".cmp(& &"1234") == Greater;
    }
}
