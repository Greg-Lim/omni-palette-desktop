#[derive(Default, Debug)]
pub struct MatchResult {
    score: i32,
    indices: Vec<usize>, // Positions of matching chars for highlighting
}

struct PreparedQuery {}

// struct Target

//Entry point for external API
// Data cleaning here
pub fn get_score(target: &String, query: &String) -> MatchResult {
    return do_score_contains(&target, &query);
}

// Filler function to get the prototype done
fn do_score_contains(target: &String, query: &String) -> MatchResult {
    dbg!(target);
    dbg!(query);

    if query.is_empty() {
        return MatchResult::default();
    }

    let target_lower = target.to_lowercase();
    let query_lower = query.to_lowercase();

    let mut indices = Vec::new();
    let mut target_chars = target_lower.char_indices().peekable();

    // 1. Filter: Sequential Character Matching
    for q_char in query_lower.chars() {
        let mut found = false;
        while let Some((idx, t_char)) = target_chars.next() {
            if t_char == q_char {
                indices.push(idx);
                found = true;
                break;
            }
        }
        // If any character in query isn't found in sequence, it's not a match
        if !found {
            return MatchResult::default();
        }
    }

    // 2. Simple Scoring (Brute Force Heuristics)
    let mut score = 100; // Base score for a match

    // Bonus: Character Proximity (Closer together = higher score)
    if let (Some(&first), Some(&last)) = (indices.first(), indices.last()) {
        let match_span = (last - first + 1) as i32;
        score -= match_span; // Penalty for large gaps
    }

    // Bonus: Start of string match
    if indices.get(0) == Some(&0) {
        score += 50;
    }

    MatchResult { score, indices }
}

// The scorring engine
fn do_score_fuzzy() {
    todo!()
}
