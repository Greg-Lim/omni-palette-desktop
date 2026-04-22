use crate::core::search::{prepare_query, score_fuzzy, MatchRange, PreparedQuery};
use crate::domain::action::{CommandPriority, FocusState};

#[derive(Debug, Clone)]
pub struct FilteredCommand {
    pub command_index: usize,
    pub score: i32,
    pub label_matches: Vec<MatchRange>,
    pub is_prefix: bool,
    pub span: usize,
}

pub trait FilterableCommand {
    fn label(&self) -> &str;
    fn priority(&self) -> CommandPriority;
    fn focus_state(&self) -> FocusState;
    fn favorite(&self) -> bool;
    fn tags(&self) -> &[String];
    fn original_order(&self) -> usize;
}

pub fn initial_filtered_commands(command_count: usize) -> Vec<FilteredCommand> {
    (0..command_count)
        .map(|command_index| FilteredCommand {
            command_index,
            score: 0,
            label_matches: Vec::new(),
            is_prefix: false,
            span: 0,
        })
        .collect()
}

pub fn filter_commands<T: FilterableCommand>(
    commands: &[T],
    filter_text: &str,
) -> Vec<FilteredCommand> {
    if filter_text.is_empty() {
        return initial_sorted_filtered_commands(commands);
    }

    let prepared_query = prepare_query(filter_text);
    let mut scored: Vec<FilteredCommand> = commands
        .iter()
        .enumerate()
        .filter_map(|(i, command)| score_command(i, command, &prepared_query))
        .collect();

    scored.sort_by(|a, b| {
        let command_a = &commands[a.command_index];
        let command_b = &commands[b.command_index];

        command_b
            .favorite()
            .cmp(&command_a.favorite())
            .then_with(|| {
                focus_rank(command_b.focus_state()).cmp(&focus_rank(command_a.focus_state()))
            })
            .then_with(|| command_b.priority().cmp(&command_a.priority()))
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| b.is_prefix.cmp(&a.is_prefix))
            .then_with(|| a.span.cmp(&b.span))
            .then_with(|| command_a.label().len().cmp(&command_b.label().len()))
            .then_with(|| compare_labels(command_a.label(), command_b.label()))
            .then_with(|| command_a.original_order().cmp(&command_b.original_order()))
    });

    scored
}

fn initial_sorted_filtered_commands<T: FilterableCommand>(commands: &[T]) -> Vec<FilteredCommand> {
    let mut rows = initial_filtered_commands(commands.len());

    rows.sort_by(|a, b| {
        let command_a = &commands[a.command_index];
        let command_b = &commands[b.command_index];

        command_b
            .favorite()
            .cmp(&command_a.favorite())
            .then_with(|| {
                focus_rank(command_b.focus_state()).cmp(&focus_rank(command_a.focus_state()))
            })
            .then_with(|| command_b.priority().cmp(&command_a.priority()))
            .then_with(|| compare_labels(command_a.label(), command_b.label()))
            .then_with(|| command_a.original_order().cmp(&command_b.original_order()))
    });

    rows
}

fn focus_rank(focus_state: FocusState) -> u8 {
    match focus_state {
        FocusState::Focused => 2,
        FocusState::Background => 1,
        FocusState::Global => 0,
    }
}

fn compare_labels(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_ascii_lowercase()
        .cmp(&b.to_ascii_lowercase())
        .then_with(|| a.cmp(b))
}

fn score_command<T: FilterableCommand>(
    command_index: usize,
    command: &T,
    query: &PreparedQuery,
) -> Option<FilteredCommand> {
    let label_match = score_fuzzy(command.label(), query);
    let tag_match = command
        .tags()
        .iter()
        .filter_map(|tag| score_fuzzy(tag, query))
        .max_by(|a, b| a.score.cmp(&b.score));

    let mut result = match (label_match, tag_match) {
        (Some(label), Some(tag)) => FilteredCommand {
            command_index,
            score: label.score + (tag.score * 3 / 10),
            label_matches: label.ranges,
            is_prefix: label.is_prefix,
            span: label.span,
        },
        (Some(label), None) => FilteredCommand {
            command_index,
            score: label.score,
            label_matches: label.ranges,
            is_prefix: label.is_prefix,
            span: label.span,
        },
        (None, Some(tag)) => FilteredCommand {
            command_index,
            score: tag.score * 3 / 5,
            label_matches: Vec::new(),
            is_prefix: false,
            span: usize::MAX,
        },
        (None, None) => return None,
    };

    result.score += word_initial_bonus(command.label(), query.normalized_lower.as_str());
    result.score += focus_bonus(command.focus_state());
    Some(result)
}

fn focus_bonus(focus_state: FocusState) -> i32 {
    match focus_state {
        FocusState::Focused => 6,
        FocusState::Background => 3,
        FocusState::Global => 1,
    }
}

fn word_initial_bonus(label: &str, query: &str) -> i32 {
    if query.is_empty() {
        return 0;
    }

    let initials: Vec<String> = label
        .char_indices()
        .filter_map(|(index, ch)| {
            if is_word_start(label, index) {
                Some(ch.to_lowercase().to_string())
            } else {
                None
            }
        })
        .collect();

    if initials.is_empty() {
        return 0;
    }

    let mut initials = initials.iter();
    for query_char in query.chars().map(|ch| ch.to_string()) {
        if !initials.any(|initial| *initial == query_char) {
            return 0;
        }
    }

    80
}

fn is_word_start(label: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return true;
    }

    let previous = label[..byte_index].chars().last();
    previous.is_some_and(|ch| {
        matches!(
            ch,
            ' ' | '\t' | ':' | '/' | '\\' | '-' | '_' | '.' | '\'' | '"'
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestCommand {
        label: String,
        priority: CommandPriority,
        focus_state: FocusState,
        favorite: bool,
        tags: Vec<String>,
        original_order: usize,
    }

    impl FilterableCommand for TestCommand {
        fn label(&self) -> &str {
            &self.label
        }

        fn priority(&self) -> CommandPriority {
            self.priority
        }

        fn focus_state(&self) -> FocusState {
            self.focus_state
        }

        fn favorite(&self) -> bool {
            self.favorite
        }

        fn tags(&self) -> &[String] {
            &self.tags
        }

        fn original_order(&self) -> usize {
            self.original_order
        }
    }

    fn command(
        label: &str,
        priority: CommandPriority,
        favorite: bool,
        tags: &[&str],
        original_order: usize,
    ) -> TestCommand {
        TestCommand {
            label: label.to_string(),
            priority,
            focus_state: FocusState::Focused,
            favorite,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            original_order,
        }
    }

    fn command_with_focus(
        label: &str,
        priority: CommandPriority,
        focus_state: FocusState,
        original_order: usize,
    ) -> TestCommand {
        TestCommand {
            label: label.to_string(),
            priority,
            focus_state,
            favorite: false,
            tags: Vec::new(),
            original_order,
        }
    }

    #[test]
    fn empty_filter_sorts_by_priority_then_label_without_scores() {
        let commands = vec![
            command("Chrome: Zoom in", CommandPriority::Medium, false, &[], 0),
            command("Chrome: Close tab", CommandPriority::High, true, &[], 1),
            command("Chrome: New tab", CommandPriority::High, false, &[], 2),
            command(
                "Chrome: Bookmark page",
                CommandPriority::Suppressed,
                false,
                &[],
                3,
            ),
        ];

        let rows = filter_commands(&commands, "");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(
            labels,
            vec![
                "Chrome: Close tab",
                "Chrome: New tab",
                "Chrome: Zoom in",
                "Chrome: Bookmark page",
            ]
        );
        assert!(rows.iter().all(|row| row.score == 0));
        assert!(rows.iter().all(|row| row.label_matches.is_empty()));
    }

    #[test]
    fn sorting_uses_favorite_then_priority_then_fuzzy_score() {
        let commands = vec![
            command("Chrome: Foo medium", CommandPriority::Medium, false, &[], 0),
            command("Chrome: Foo high", CommandPriority::High, false, &[], 1),
            command(
                "Chrome: Foo suppressed",
                CommandPriority::Suppressed,
                false,
                &[],
                2,
            ),
            command(
                "Chrome: Foo favorite suppressed",
                CommandPriority::Suppressed,
                true,
                &[],
                3,
            ),
            command(
                "Chrome: Foo favorite high",
                CommandPriority::High,
                true,
                &[],
                4,
            ),
        ];

        let rows = filter_commands(&commands, "foo");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(
            labels,
            vec![
                "Chrome: Foo favorite high",
                "Chrome: Foo favorite suppressed",
                "Chrome: Foo high",
                "Chrome: Foo medium",
                "Chrome: Foo suppressed",
            ]
        );
    }

    #[test]
    fn searched_results_use_alphabetical_order_as_final_tiebreaker() {
        let commands = vec![
            command(
                "Chrome: Switch to tab 8",
                CommandPriority::Suppressed,
                false,
                &[],
                0,
            ),
            command(
                "Chrome: Switch to tab 3",
                CommandPriority::Suppressed,
                false,
                &[],
                1,
            ),
            command(
                "Chrome: Switch to tab 1",
                CommandPriority::Suppressed,
                false,
                &[],
                2,
            ),
        ];

        let rows = filter_commands(&commands, "sw");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(
            labels,
            vec![
                "Chrome: Switch to tab 1",
                "Chrome: Switch to tab 3",
                "Chrome: Switch to tab 8",
            ]
        );
    }

    #[test]
    fn searched_results_keep_priority_before_fuzzy_quality() {
        let commands = vec![
            command(
                "Chrome: Switch to tab 1",
                CommandPriority::Suppressed,
                false,
                &[],
                0,
            ),
            command(
                "Chrome: Scroll down",
                CommandPriority::Medium,
                false,
                &[],
                1,
            ),
            command(
                "Chrome: Switch to last tab",
                CommandPriority::High,
                false,
                &[],
                2,
            ),
        ];

        let rows = filter_commands(&commands, "sw");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(
            labels,
            vec![
                "Chrome: Switch to last tab",
                "Chrome: Scroll down",
                "Chrome: Switch to tab 1",
            ]
        );
    }

    #[test]
    fn focused_commands_rank_above_global_commands_before_priority() {
        let commands = vec![
            command_with_focus(
                "Windows: Open File Explorer",
                CommandPriority::High,
                FocusState::Global,
                0,
            ),
            command_with_focus(
                "Chrome: Open file",
                CommandPriority::Suppressed,
                FocusState::Focused,
                1,
            ),
        ];

        let rows = filter_commands(&commands, "file");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(
            labels,
            vec!["Chrome: Open file", "Windows: Open File Explorer"]
        );
    }

    #[test]
    fn empty_filter_ranks_focused_commands_above_global_commands_before_priority() {
        let commands = vec![
            command_with_focus(
                "Windows: Open File Explorer",
                CommandPriority::High,
                FocusState::Global,
                0,
            ),
            command_with_focus(
                "Chrome: Open file",
                CommandPriority::Suppressed,
                FocusState::Focused,
                1,
            ),
        ];

        let rows = filter_commands(&commands, "");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(
            labels,
            vec!["Chrome: Open file", "Windows: Open File Explorer"]
        );
    }

    #[test]
    fn tag_only_matches_are_included_without_label_highlights() {
        let commands = vec![command(
            "Chrome: Open Developer Tools",
            CommandPriority::Medium,
            false,
            &["debug"],
            0,
        )];

        let rows = filter_commands(&commands, "debug");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command_index, 0);
        assert!(rows[0].label_matches.is_empty());
        assert!(rows[0].score > 0);
    }

    #[test]
    fn non_matching_commands_are_excluded() {
        let commands = vec![command(
            "Chrome: Open Developer Tools",
            CommandPriority::Medium,
            false,
            &["debug"],
            0,
        )];

        let rows = filter_commands(&commands, "banana");

        assert!(rows.is_empty());
    }

    #[test]
    fn word_initial_acronym_ranks_reload_page_high_for_rp() {
        let commands = vec![
            command(
                "Chrome: Reopen closed tab",
                CommandPriority::Medium,
                false,
                &[],
                0,
            ),
            command(
                "Chrome: Reload page",
                CommandPriority::Medium,
                false,
                &[],
                1,
            ),
            command(
                "Chrome: Open Developer Tools",
                CommandPriority::Medium,
                false,
                &[],
                2,
            ),
            command("Chrome: Reset zoom", CommandPriority::Medium, false, &[], 3),
        ];

        let rows = filter_commands(&commands, "rp");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(labels.first(), Some(&"Chrome: Reload page"));
        assert!(
            labels
                .iter()
                .position(|label| *label == "Chrome: Reload page")
                < labels
                    .iter()
                    .position(|label| *label == "Chrome: Reopen closed tab")
        );
    }

    #[test]
    fn word_initial_order_matters_for_acronym_queries() {
        let commands = vec![
            command(
                "Chrome: Reload page",
                CommandPriority::Medium,
                false,
                &[],
                0,
            ),
            command(
                "Chrome: Page reload",
                CommandPriority::Medium,
                false,
                &[],
                1,
            ),
            command("Chrome: Print page", CommandPriority::Medium, false, &[], 2),
        ];

        let rp_rows = filter_commands(&commands, "rp");
        let pr_rows = filter_commands(&commands, "pr");

        assert_eq!(
            commands[rp_rows[0].command_index].label,
            "Chrome: Reload page"
        );
        assert_eq!(
            commands[pr_rows[0].command_index].label,
            "Chrome: Page reload"
        );
    }

    #[test]
    fn reload_page_initials_rank_above_print_page_for_rp() {
        let commands = vec![
            command("Chrome: Print page", CommandPriority::Medium, false, &[], 0),
            command(
                "Chrome: Reload page",
                CommandPriority::Medium,
                false,
                &[],
                1,
            ),
        ];

        let rows = filter_commands(&commands, "rp");
        let reload_page = rows
            .iter()
            .find(|row| commands[row.command_index].label == "Chrome: Reload page")
            .expect("Reload page should match rp");
        let print_page = rows
            .iter()
            .find(|row| commands[row.command_index].label == "Chrome: Print page")
            .expect("Print page should match rp");

        assert_eq!(commands[rows[0].command_index].label, "Chrome: Reload page");

        // score_command should give a significant bonus to "Reload page" for matching the initials "R" and "P",
        // even though "Print page" also contains the query characters "r" and "p".
        assert!(reload_page.score >= print_page.score + 40);
    }

    #[test]
    fn chrome_rp_results_put_reload_page_above_print_and_previous_find() {
        let commands = vec![
            command(
                "Chrome: Previous find match",
                CommandPriority::Medium,
                false,
                &[],
                0,
            ),
            command("Chrome: Print page", CommandPriority::Medium, false, &[], 1),
            command(
                "Chrome: Reload page",
                CommandPriority::Medium,
                false,
                &[],
                2,
            ),
            command(
                "Chrome: Reload (ignore cache)",
                CommandPriority::Medium,
                false,
                &[],
                3,
            ),
        ];

        let rows = filter_commands(&commands, "rp");
        let labels: Vec<&str> = rows
            .iter()
            .map(|row| commands[row.command_index].label.as_str())
            .collect();

        assert_eq!(labels.first(), Some(&"Chrome: Reload page"));
        assert!(
            labels
                .iter()
                .position(|label| *label == "Chrome: Reload page")
                < labels
                    .iter()
                    .position(|label| *label == "Chrome: Print page")
        );
        assert!(
            labels
                .iter()
                .position(|label| *label == "Chrome: Reload page")
                < labels
                    .iter()
                    .position(|label| *label == "Chrome: Previous find match")
        );
    }

    #[test]
    fn close_query_highlights_contiguous_close_word_not_app_prefix() {
        let commands = vec![
            command("Chrome: Close tab", CommandPriority::Medium, false, &[], 0),
            command(
                "Chrome: Close window",
                CommandPriority::Medium,
                false,
                &[],
                1,
            ),
            command(
                "Chrome: Reopen closed tab",
                CommandPriority::Medium,
                false,
                &[],
                2,
            ),
        ];

        let rows = filter_commands(&commands, "close");
        let first = &rows[0];
        let first_label = commands[first.command_index].label.as_str();
        let expected_start = first_label
            .find("Close")
            .expect("label should contain Close");
        let expected_end = expected_start + "Close".len();

        assert_eq!(first_label, "Chrome: Close tab");
        assert_eq!(
            first.label_matches,
            vec![MatchRange {
                start: expected_start,
                end: expected_end,
            }]
        );
    }
}
