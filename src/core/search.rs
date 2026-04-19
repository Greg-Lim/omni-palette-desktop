#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub score: i32,
    pub ranges: Vec<MatchRange>,
    pub is_prefix: bool,
    pub span: usize,
}

#[derive(Debug, Clone)]
pub struct PreparedQuery {
    pub normalized: String,
    pub normalized_lower: String,
    pub pieces: Vec<QueryPiece>,
    pub expect_contiguous_match: bool,
}

#[derive(Debug, Clone)]
pub struct QueryPiece {
    pub normalized: String,
    pub normalized_lower: String,
    pub expect_contiguous_match: bool,
}

#[derive(Debug, Clone)]
struct TargetChar {
    ch: char,
    lower: String,
    byte_start: usize,
    byte_end: usize,
}

const NO_MATCH: usize = 0;

#[cfg(test)]
pub fn get_score(target: &str, query: &str) -> Option<MatchResult> {
    let prepared = prepare_query(query);
    score_fuzzy(target, &prepared)
}

pub fn prepare_query(query: &str) -> PreparedQuery {
    let original = query.trim().to_string();
    let expect_contiguous_match = query_expects_exact_match(&original);
    let normalized = if expect_contiguous_match {
        strip_exact_quotes(&original).to_string()
    } else {
        normalize_query(&original)
    };
    let normalized_lower = normalized.to_lowercase();

    let pieces = if expect_contiguous_match {
        Vec::new()
    } else {
        original
            .split_whitespace()
            .filter_map(|piece| {
                let normalized = normalize_query(piece);
                if normalized.is_empty() {
                    return None;
                }

                Some(QueryPiece {
                    normalized_lower: normalized.to_lowercase(),
                    normalized,
                    expect_contiguous_match: query_expects_exact_match(piece),
                })
            })
            .collect()
    };

    PreparedQuery {
        normalized,
        normalized_lower,
        pieces,
        expect_contiguous_match,
    }
}

pub fn score_fuzzy(target: &str, query: &PreparedQuery) -> Option<MatchResult> {
    if target.is_empty() || query.normalized.is_empty() {
        return None;
    }

    if query.pieces.len() > 1 {
        let mut score = 0;
        let mut ranges = Vec::new();
        let mut is_prefix = false;

        for piece in &query.pieces {
            let result = score_piece(target, piece)?;
            score += result.score;
            is_prefix |= result.is_prefix;
            ranges.extend(result.ranges);
        }

        let ranges = normalize_ranges(ranges);
        let span = match_span(&ranges);

        return Some(MatchResult {
            score,
            ranges,
            is_prefix,
            span,
        });
    }

    let piece = QueryPiece {
        normalized: query.normalized.clone(),
        normalized_lower: query.normalized_lower.clone(),
        expect_contiguous_match: query.expect_contiguous_match,
    };

    score_piece(target, &piece)
}

fn score_piece(target: &str, query: &QueryPiece) -> Option<MatchResult> {
    if query.normalized.is_empty() {
        return None;
    }

    if query.expect_contiguous_match {
        return score_contiguous(target, query);
    }

    score_fuzzy_piece(target, query)
}

fn score_fuzzy_piece(target: &str, query: &QueryPiece) -> Option<MatchResult> {
    let target_chars = target_chars(target);
    let query_chars: Vec<char> = query.normalized.chars().collect();
    let query_lower: Vec<String> = query
        .normalized_lower
        .chars()
        .map(|ch| ch.to_string())
        .collect();

    let target_len = target_chars.len();
    let query_len = query_chars.len();

    if target_len < query_len || query_len == 0 {
        return None;
    }

    let mut scores = vec![0; target_len * query_len];
    let mut matches = vec![NO_MATCH; target_len * query_len];

    for query_index in 0..query_len {
        let row_offset = query_index * target_len;
        let previous_row_offset = row_offset.wrapping_sub(target_len);
        let query_has_previous = query_index > 0;

        for target_index in 0..target_len {
            let current_index = row_offset + target_index;
            let target_has_previous = target_index > 0;
            let left_score = if target_has_previous {
                scores[current_index - 1]
            } else {
                0
            };
            let diagonal_score = if query_has_previous && target_has_previous {
                scores[previous_row_offset + target_index - 1]
            } else {
                0
            };
            let sequence_len = if query_has_previous && target_has_previous {
                matches[previous_row_offset + target_index - 1]
            } else {
                0
            };

            let char_score = if query_has_previous && diagonal_score == 0 {
                0
            } else {
                compute_char_score(
                    query_chars[query_index],
                    &query_lower[query_index],
                    &target_chars,
                    target_index,
                    sequence_len,
                )
            };

            if char_score > 0 && diagonal_score + char_score >= left_score {
                matches[current_index] = sequence_len + 1;
                scores[current_index] = diagonal_score + char_score;
            } else {
                matches[current_index] = NO_MATCH;
                scores[current_index] = left_score;
            }
        }
    }

    let score = scores[target_len * query_len - 1];
    if score == 0 {
        return None;
    }

    let mut positions = Vec::new();
    let mut query_index = query_len;
    let mut target_index = target_len;

    while query_index > 0 && target_index > 0 {
        let current_index = (query_index - 1) * target_len + (target_index - 1);
        if matches[current_index] == NO_MATCH {
            target_index -= 1;
        } else {
            positions.push(target_index - 1);
            query_index -= 1;
            target_index -= 1;
        }
    }

    if positions.len() != query_len {
        return None;
    }

    positions.reverse();
    let ranges = positions_to_ranges(&target_chars, &positions);
    let span = match_span(&ranges);
    let is_prefix = target
        .to_lowercase()
        .starts_with(query.normalized_lower.as_str());

    let fuzzy_result = MatchResult {
        score,
        ranges,
        is_prefix,
        span,
    };

    match score_word_prefix(target, query) {
        Some(word_prefix) if word_prefix.score > fuzzy_result.score => Some(word_prefix),
        _ => Some(fuzzy_result),
    }
}

fn score_contiguous(target: &str, query: &QueryPiece) -> Option<MatchResult> {
    let target_chars = target_chars(target);
    let query_lower: Vec<String> = query
        .normalized_lower
        .chars()
        .map(|ch| ch.to_string())
        .collect();

    if query_lower.is_empty() || target_chars.len() < query_lower.len() {
        return None;
    }

    for start in 0..=target_chars.len() - query_lower.len() {
        let matches = query_lower
            .iter()
            .enumerate()
            .all(|(offset, query_char)| target_chars[start + offset].lower == *query_char);

        if matches {
            let byte_start = target_chars[start].byte_start;
            let byte_end = target_chars[start + query_lower.len() - 1].byte_end;
            let ranges = vec![MatchRange {
                start: byte_start,
                end: byte_end,
            }];

            return Some(MatchResult {
                score: 200 + query_lower.len() as i32,
                ranges,
                is_prefix: start == 0,
                span: byte_end - byte_start,
            });
        }
    }

    None
}

fn score_word_prefix(target: &str, query: &QueryPiece) -> Option<MatchResult> {
    let target_chars = target_chars(target);
    let query_lower: Vec<String> = query
        .normalized_lower
        .chars()
        .map(|ch| ch.to_string())
        .collect();

    if query_lower.len() < 3 || target_chars.len() < query_lower.len() {
        return None;
    }

    for start in 0..=target_chars.len() - query_lower.len() {
        if !is_word_start(&target_chars, start) {
            continue;
        }

        let matches = query_lower
            .iter()
            .enumerate()
            .all(|(offset, query_char)| target_chars[start + offset].lower == *query_char);

        if matches {
            let end_index = start + query_lower.len() - 1;
            let byte_start = target_chars[start].byte_start;
            let byte_end = target_chars[end_index].byte_end;
            let is_full_word = end_index + 1 == target_chars.len()
                || separator_bonus(target_chars[end_index + 1].ch).is_some();
            let score = 80
                + (query_lower.len() as i32 * 10)
                + if is_full_word { 30 } else { 0 }
                + if start == 0 { 8 } else { 0 };

            return Some(MatchResult {
                score,
                ranges: vec![MatchRange {
                    start: byte_start,
                    end: byte_end,
                }],
                is_prefix: start == 0,
                span: byte_end - byte_start,
            });
        }
    }

    None
}

fn is_word_start(target: &[TargetChar], index: usize) -> bool {
    index == 0 || separator_bonus(target[index - 1].ch).is_some()
}

fn compute_char_score(
    query_char: char,
    query_lower: &str,
    target: &[TargetChar],
    target_index: usize,
    sequence_len: usize,
) -> i32 {
    let target_char = &target[target_index];
    if query_lower != target_char.lower {
        return 0;
    }

    let mut score = 1;

    if query_char == target_char.ch {
        score += 1;
    }

    if sequence_len > 0 {
        score += (sequence_len.min(3) as i32 * 6) + (sequence_len.saturating_sub(3) as i32 * 3);
    }

    if target_index == 0 {
        score += 8;
    } else if let Some(separator_bonus) = separator_bonus(target[target_index - 1].ch) {
        score += separator_bonus;
    } else if target_char.ch.is_uppercase() && sequence_len == 0 {
        score += 2;
    }

    score
}

fn separator_bonus(ch: char) -> Option<i32> {
    match ch {
        '/' | '\\' => Some(5),
        '_' | '-' | '.' | ' ' | '\'' | '"' | ':' => Some(4),
        _ => None,
    }
}

fn target_chars(target: &str) -> Vec<TargetChar> {
    let mut iter = target.char_indices().peekable();
    let mut chars = Vec::new();

    while let Some((byte_start, ch)) = iter.next() {
        let byte_end = iter.peek().map(|(idx, _)| *idx).unwrap_or(target.len());
        chars.push(TargetChar {
            ch,
            lower: ch.to_lowercase().to_string(),
            byte_start,
            byte_end,
        });
    }

    chars
}

fn positions_to_ranges(target: &[TargetChar], positions: &[usize]) -> Vec<MatchRange> {
    let mut ranges: Vec<MatchRange> = Vec::new();

    for &position in positions {
        let ch = &target[position];
        if let Some(last) = ranges.last_mut() {
            if last.end == ch.byte_start {
                last.end = ch.byte_end;
                continue;
            }
        }

        ranges.push(MatchRange {
            start: ch.byte_start,
            end: ch.byte_end,
        });
    }

    ranges
}

fn normalize_ranges(mut ranges: Vec<MatchRange>) -> Vec<MatchRange> {
    ranges.sort_by_key(|range| range.start);

    let mut normalized: Vec<MatchRange> = Vec::new();
    for range in ranges {
        if let Some(last) = normalized.last_mut() {
            if range.start <= last.end {
                last.end = last.end.max(range.end);
                continue;
            }
        }

        normalized.push(range);
    }

    normalized
}

fn match_span(ranges: &[MatchRange]) -> usize {
    match (ranges.first(), ranges.last()) {
        (Some(first), Some(last)) => last.end - first.start,
        _ => 0,
    }
}

fn normalize_query(query: &str) -> String {
    query
        .chars()
        .filter(|ch| !matches!(ch, '*' | '\u{2026}' | '"' | '\'' | ' '))
        .collect()
}

fn query_expects_exact_match(query: &str) -> bool {
    query.len() >= 2 && query.starts_with('"') && query.ends_with('"')
}

fn strip_exact_quotes(query: &str) -> &str {
    if query_expects_exact_match(query) {
        &query[1..query.len() - 1]
    } else {
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_succeeds_for_ordered_chars() {
        let result = get_score("Chrome: New tab", "nt").unwrap();
        assert!(result.score > 0);
        assert!(!result.ranges.is_empty());
    }

    #[test]
    fn wrong_order_does_not_match() {
        assert!(get_score("Chrome: New tab", "zn").is_none());
    }

    #[test]
    fn separator_match_beats_inside_word_match() {
        let after_separator = get_score("New tab", "t").unwrap();
        let inside_word = get_score("Notable", "t").unwrap();
        assert!(after_separator.score > inside_word.score);
    }

    #[test]
    fn camel_case_boundaries_score() {
        let result = get_score("Chrome: DeveloperTools", "dt").unwrap();
        assert!(result.score > 0);
    }

    #[test]
    fn multiple_pieces_must_all_match() {
        assert!(get_score("Chrome: New tab", "chrome tab").is_some());
        assert!(get_score("Chrome: New tab", "chrome banana").is_none());
    }

    #[test]
    fn quoted_query_is_contiguous() {
        assert!(get_score("Chrome: New tab", "\"new tab\"").is_some());
        assert!(get_score("Chrome: New tab", "\"tab new\"").is_none());
    }

    #[test]
    fn consecutive_ranges_are_merged() {
        let result = get_score("Chrome: New tab", "new").unwrap();
        assert!(result
            .ranges
            .iter()
            .any(|range| range.end - range.start == 3));
    }

    #[test]
    fn unicode_ranges_are_valid_byte_ranges() {
        let target = "Cafe: R\u{e9}sum\u{e9}";
        let result = get_score(target, "r\u{e9}").unwrap();

        for range in result.ranges {
            assert!(target.get(range.start..range.end).is_some());
        }
    }
}
