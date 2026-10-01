//! Text search (V-16): finds a query in the document's lines. Smart case:
//! a query with a capital letter is case-sensitive, otherwise not.
//! Positions are (line, char column).

/// The search prompt's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    pub query: String,
    /// Where the cursor was when the search started (Esc goes back).
    pub origin: (usize, usize),
}

/// The find-and-replace prompt's state (V-17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replace {
    pub find: String,
    pub with: String,
    /// Whether typing goes into the replacement (Tab switches fields).
    pub editing_with: bool,
    /// Where the cursor was when the prompt opened.
    pub origin: (usize, usize),
}

/// The next match of `query` from `from`, wrapping around the document.
/// Forward searches include a match starting exactly at `from` when
/// `inclusive`; backward searches look before `from`.
pub fn find(
    lines: &[String],
    query: &str,
    from: (usize, usize),
    forward: bool,
    inclusive: bool,
) -> Option<(usize, usize)> {
    let all = matches(lines, query);
    if forward {
        all.iter()
            .find(|&&m| m > from || (inclusive && m == from))
            .or(all.first())
            .copied()
    } else {
        all.iter()
            .rev()
            .find(|&&m| m < from)
            .or(all.last())
            .copied()
    }
}

/// Every match of `query`, in document order.
pub fn matches(lines: &[String], query: &str) -> Vec<(usize, usize)> {
    lines
        .iter()
        .enumerate()
        .flat_map(|(row, line)| {
            line_matches(line, query)
                .into_iter()
                .map(move |col| (row, col))
        })
        .collect()
}

/// Char columns where `query` starts in `line` (overlapping matches too).
fn line_matches(line: &str, query: &str) -> Vec<usize> {
    let exact = query.chars().any(char::is_uppercase);
    // ASCII (most notes): bytes are chars, compared in place.
    if line.is_ascii() && query.is_ascii() {
        let (hay, needle) = (line.as_bytes(), query.as_bytes());
        if needle.is_empty() || needle.len() > hay.len() {
            return Vec::new();
        }
        let same = |a: &[u8]| {
            if exact {
                a == needle
            } else {
                a.eq_ignore_ascii_case(needle)
            }
        };
        let first = needle[0].to_ascii_lowercase();
        return (0..=hay.len() - needle.len())
            .filter(|&i| {
                let b = hay[i];
                (if exact {
                    b == needle[0]
                } else {
                    b.to_ascii_lowercase() == first
                }) && same(&hay[i..i + needle.len()])
            })
            .collect();
    }
    // One char in, one char out, so columns match the original text.
    let fold = move |c: char| {
        if exact {
            c
        } else {
            c.to_lowercase().next().unwrap_or(c)
        }
    };
    let needle: Vec<char> = query.chars().map(fold).collect();
    let hay: Vec<char> = line.chars().map(fold).collect();
    let last = if needle.is_empty() {
        0
    } else {
        (hay.len() + 1).saturating_sub(needle.len())
    };
    (0..last)
        .filter(|&col| hay[col..col + needle.len()] == needle[..])
        .collect()
}

/// The char ranges where `query` appears in `text`, left to right and not
/// overlapping (for highlighting matches on screen, V-20). Smart case like
/// [`matches`].
pub fn highlights(text: &str, query: &str) -> Vec<(usize, usize)> {
    let len = query.chars().count();
    let mut next = 0;
    line_matches(text, query)
        .into_iter()
        .filter(|&col| {
            let free = col >= next;
            if free {
                next = col + len;
            }
            free
        })
        .map(|col| (col, col + len))
        .collect()
}

/// Byte index of char column `col` in `line`.
fn byte_at(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(b, _)| b)
}

/// "3/12"-style position of `at` among the matches, or "No match".
pub fn describe(lines: &[String], query: &str, at: (usize, usize)) -> String {
    let all = matches(lines, query);
    match all.iter().position(|&m| m == at) {
        Some(i) => format!("{}/{}", i + 1, all.len()),
        None if all.is_empty() => "No match".to_string(),
        None => format!("{} matches", all.len()),
    }
}

/// Replaces the match of `query` at `at` with `with`, if there is one
/// there. Returns the position just after the replacement.
pub fn replace_at(
    lines: &mut [String],
    query: &str,
    at: (usize, usize),
    with: &str,
) -> Option<(usize, usize)> {
    let (row, col) = at;
    let line = lines.get_mut(row)?;
    if !line_matches(line, query).contains(&col) {
        return None;
    }
    let (start, end) = (
        byte_at(line, col),
        byte_at(line, col + query.chars().count()),
    );
    line.replace_range(start..end, with);
    Some((row, col + with.chars().count()))
}

/// Replaces every match of `query` with `with` (left to right, matches
/// don't overlap). Returns how many were replaced.
pub fn replace_all(lines: &mut [String], query: &str, with: &str) -> usize {
    let len = query.chars().count();
    let mut count = 0;
    for line in lines.iter_mut() {
        let starts: Vec<usize> = highlights(line, query).iter().map(|&(a, _)| a).collect();
        // Right to left, so earlier columns stay valid.
        for &col in starts.iter().rev() {
            let (start, end) = (byte_at(line, col), byte_at(line, col + len));
            line.replace_range(start..end, with);
        }
        count += starts.len();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn finds_all_matches_in_order() {
        let d = doc("a beta\nbeta beta\nnone");
        assert_eq!(matches(&d, "beta"), [(0, 2), (1, 0), (1, 5)]);
        assert!(matches(&d, "").is_empty());
    }

    #[test]
    fn smart_case() {
        let d = doc("Beta beta");
        assert_eq!(matches(&d, "beta"), [(0, 0), (0, 5)], "lowercase: any case");
        assert_eq!(matches(&d, "Beta"), [(0, 0)], "a capital: exact case");
    }

    #[test]
    fn forward_inclusive_or_after_and_wraps() {
        let d = doc("x beta\nbeta");
        assert_eq!(find(&d, "beta", (0, 2), true, true), Some((0, 2)));
        assert_eq!(find(&d, "beta", (0, 2), true, false), Some((1, 0)));
        assert_eq!(find(&d, "beta", (1, 0), true, false), Some((0, 2)), "wraps");
    }

    #[test]
    fn backward_and_wraps() {
        let d = doc("x beta\nbeta");
        assert_eq!(find(&d, "beta", (1, 0), false, false), Some((0, 2)));
        assert_eq!(
            find(&d, "beta", (0, 2), false, false),
            Some((1, 0)),
            "wraps"
        );
    }

    #[test]
    fn edges_match_like_the_general_search() {
        let d = doc("aaa\ncafÉ x\nAB ab");
        assert_eq!(matches(&d, "aa"), [(0, 0), (0, 1)], "overlapping");
        assert!(matches(&d, "").is_empty());
        assert!(matches(&d, "aaaa").is_empty(), "longer than the line");
        assert_eq!(matches(&d, "é"), [(1, 3)], "non-ASCII query, any case");
        assert_eq!(matches(&d, "AB"), [(2, 0)], "uppercase: exact");
        assert_eq!(matches(&d, "ab"), [(2, 0), (2, 3)]);
    }

    #[test]
    fn columns_are_chars() {
        let d = doc("é😀 beta");
        assert_eq!(matches(&d, "beta"), [(0, 3)]);
    }

    #[test]
    fn replace_at_a_match_only() {
        let mut d = doc("a cat, a cat");
        assert_eq!(replace_at(&mut d, "cat", (0, 2), "dog"), Some((0, 5)));
        assert_eq!(d, doc("a dog, a cat"));
        assert_eq!(
            replace_at(&mut d, "cat", (0, 0), "dog"),
            None,
            "no match there"
        );
        assert_eq!(d, doc("a dog, a cat"));
    }

    #[test]
    fn replace_all_counts_and_never_overlaps() {
        let mut d = doc("aaa\nxaax");
        assert_eq!(replace_all(&mut d, "aa", "b"), 2);
        assert_eq!(d, doc("ba\nxbx"));
    }

    #[test]
    fn replace_all_uses_smart_case_and_multibyte_columns() {
        let mut d = doc("Cat cat é cat");
        assert_eq!(replace_all(&mut d, "cat", "🐈"), 3);
        assert_eq!(d, doc("🐈 🐈 é 🐈"));
        let mut d = doc("Cat cat");
        assert_eq!(replace_all(&mut d, "Cat", "x"), 1, "a capital: exact case");
        assert_eq!(d, doc("x cat"));
    }

    #[test]
    fn replacement_containing_the_query_does_not_loop() {
        let mut d = doc("cat cat");
        assert_eq!(replace_all(&mut d, "cat", "catcat"), 2);
        assert_eq!(d, doc("catcat catcat"));
    }

    #[test]
    fn highlights_are_char_ranges_that_do_not_overlap() {
        assert_eq!(highlights("An embed, EMBEDS", "embed"), [(3, 8), (10, 15)]);
        assert_eq!(
            highlights("An embed, EMBEDS", "Embed"),
            [],
            "a capital: exact case"
        );
        assert_eq!(highlights("aaaa", "aa"), [(0, 2), (2, 4)]);
        assert_eq!(highlights("é😀 x", "x"), [(3, 4)]);
        assert_eq!(highlights("text", ""), []);
    }

    #[test]
    fn no_match() {
        assert_eq!(find(&doc("abc"), "zz", (0, 0), true, true), None);
    }
}
