# Fuzzy Scorer Design

Source reference: https://github.com/microsoft/vscode/blob/main/src/vs/base/common/fuzzyScorer.ts

## Goal

Replace the current prototype contains/sequential matcher in `src/core/search.rs` with a VS Code-inspired fuzzy scorer that can:

- return fuzzy matches when query characters appear in order, even when they are not adjacent;
- rank commands by match quality and command priority;
- return match ranges that the egui UI can highlight in the result list while the user types;
- keep the UI responsive for every keypress in the command palette.

This should be an adaptation of VS Code's ideas, not a direct port. Global Palette has simpler search targets than VS Code because commands are currently labels like `Chrome: New tab`, plus optional shortcut text and future metadata.

## Current State

- `src/core/search.rs` exposes `get_score(target, query) -> MatchResult`.
- `MatchResult` has `score` and `indices`, but the UI only uses `score`.
- `CommandPaletteApp::recompute_filter()` calls `get_score(&cmd.label, &filter_text)` and sorts by score descending.
- `draw_command_row()` renders labels as plain text, so match positions are lost before drawing.
- A `Priority` enum exists in config/action models today, but it represents broad source layers. This design should replace command ranking priority with a simpler three-tier model: `High`, `Normal`, and `Suppressed`.

## Product Behavior

### Search

Typing `nt` should match commands such as:

- `Chrome: New tab`
- `Chrome: New Incognito window`

Typing `ct` should match:

- `Chrome: Close tab`
- `Chrome: Chrome Task Manager`

Typing `devtools`, `dt`, or `odt` should be able to find `Chrome: Open Developer Tools`.

Empty query should keep the existing default ordering. It should not mark every command as a highlighted match.

Quoted query can later be used for exact/contiguous matching, following the same idea as VS Code. Example: `"new tab"` means the normalized target must contain `new tab` contiguously. This is useful but can be phase 2.

### Prioritizing

Sorting should combine:

- match quality from the fuzzy scorer;
- command priority from extension config or user overrides;
- star state;
- context fit from `FocusState`;
- stable fallback order for deterministic results.

Proposed order:

1. Star bucket: starred commands first, then unstarred commands.
2. Priority bucket inside each star bucket: `High`, then `Normal`, then `Suppressed`.
3. Higher fuzzy score inside the same star and priority bucket.
4. Focus state, tag match score, and other bounded metadata inside the same star and priority bucket.
5. Label prefix matches before non-prefix matches.
6. More compact match spans before scattered matches.
7. Shorter labels before longer labels.
8. Original registry order as final tie-breaker.

Star is the strongest ordering bucket. A starred command should appear above any unstarred command that passes filtering, even if the unstarred command has a higher priority or better fuzzy score.

Priority is the next hard ordering bucket, not a numeric boost:

- `High`: commands the user or extension author wants surfaced first.
- `Normal`: default command priority.
- `Suppressed`: commands that are valid but should stay low in the list.

Suppressed commands must never sort above Normal commands within the same star bucket. Enforce this by sorting on star first, then priority bucket before fuzzy score, focus state, tag match score, or any other ranking signal. A starred Suppressed command can appear above an unstarred Normal command because star state wins first, but an unstarred Suppressed command cannot appear above an unstarred Normal command.

Within the same star and priority bucket, use a bounded score:

```text
intra_bucket_score = fuzzy_score + focus_state_bonus + tag_match_bonus
```

Initial values:

```text
star bucket: starred = 1, unstarred = 0
priority bucket: High = 2, Normal = 1, Suppressed = 0
focused command: +6
background command: +3
global command: +1
```

Final sort key:

```text
(star_bucket, priority_bucket, intra_bucket_score, is_prefix, compactness, shorter_label, original_order)
```

### Highlighting

Highlight matching characters in command results, similar to VS Code's command palette results.

Assumption: this means highlighting matched text inside result rows while the user types in the search bar, not coloring the typed query inside the input itself. If literal input text highlighting is desired later, egui's `TextEdit::layouter` can be added as a separate UI task.

The scorer should return normalized match ranges over the original label. The UI can then render:

- muted app prefix, e.g. `Chrome:`;
- primary action text;
- highlighted spans for fuzzy-matched characters;
- shortcut text unchanged.

Use ranges instead of individual indices in the UI so consecutive matches are drawn as one highlighted span.

## VS Code Concepts To Adapt

The referenced VS Code scorer uses a few important ideas that map well to Global Palette:

- sequential matching: every query character must match in order;
- dynamic scoring matrix: evaluate possible character alignments instead of taking the first sequential match;
- bonuses for strong human-intent positions:
  - first character / start of target;
  - after separators like space, dash, underscore, period, slash, colon;
  - camel-case uppercase transitions;
  - consecutive matches;
  - exact-case matches;
- query preparation:
  - lowercase copy;
  - path separator normalization;
  - optional multi-word query pieces;
  - optional exact/contiguous matching when wrapped in quotes;
- result match normalization:
  - convert individual matched positions into contiguous ranges;
  - merge overlapping ranges.

Global Palette does not need file path identity scoring yet. Keep that out unless commands later gain filesystem targets.

## Search API

Replace the current API with a richer result while keeping call sites simple:

```rust
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
    pub original: String,
    pub normalized: String,
    pub normalized_lower: String,
    pub pieces: Vec<QueryPiece>,
    pub expect_contiguous_match: bool,
}

pub fn prepare_query(query: &str) -> PreparedQuery;
pub fn score_fuzzy(target: &str, query: &PreparedQuery) -> Option<MatchResult>;
```

Use `Option<MatchResult>` internally instead of returning a default score of zero for no match. The UI can still convert `None` into "not included".

Important Rust detail: byte offsets and character offsets are different. The scorer can work in character indices for the matrix, but `MatchRange` should use byte offsets into the original string so egui text slicing is safe and straightforward. Build a `Vec<(byte_index, char)>` for each target and map matched char positions back to byte ranges.

## Scoring Algorithm

Phase 1 should implement the primary VS Code-style scorer:

1. Prepare query.
2. Reject if query is empty or normalized query is longer than target by character count.
3. Build lowercase target characters once.
4. Use dynamic programming over `query_chars x target_chars`.
5. For every matching character, compute a character score:
   - `+1` for character match;
   - `+1` for exact-case match;
   - start-of-target bonus;
   - separator bonus when previous target char is a separator;
   - uppercase/camel-case bonus when not already in a consecutive sequence;
   - consecutive-sequence bonus.
6. Keep the best path through the matrix.
7. Backtrack the matrix to recover matched character positions.
8. Convert positions to `MatchRange`.

Suggested initial weights:

```text
character match:        +1
same case:              +1
target start:           +8
after slash/backslash:  +5
after common separator: +4
camel case boundary:    +2
consecutive match:      +6 for first 3 continuation chars, then +3
```

These values intentionally mirror the shape of VS Code's scoring. We can tune them after seeing real command lists.

## Query Preparation

Implement `prepare_query()` before the DP scorer:

- trim leading/trailing whitespace;
- lowercase once;
- split multiple query pieces by spaces;
- remove wildcard `*`, ellipsis, and quotes from normalized fuzzy pieces;
- preserve original query for display/debugging;
- set `expect_contiguous_match` when a query or query piece is wrapped in quotes.

Multiple query pieces should all be required to match. Example: `chrome tab` should match only commands that can match both `chrome` and `tab`. Total score is the sum of piece scores, and match ranges are merged.

Exact/contiguous pieces can initially use case-insensitive substring search and return one contiguous range.

## UI Integration Plan

### Module Ownership

Keep search behavior out of `src/ui`:

- `src/core/search.rs`: query preparation, fuzzy scoring, and match ranges.
- `src/core/command_filter.rs`: command-level filtering, priority/star/tag ordering, and filter tests.
- `src/ui/ui_main.rs`: egui rendering, keyboard navigation, command execution, and label highlighting.

The UI should call `filter_commands()` and render the returned `FilteredCommand` rows. It should not know how fuzzy scores, tags, priorities, or initials bonuses are calculated.

### Data Model

Change `CommandPaletteApp` from storing only filtered indices to storing scored rows:

```rust
pub struct FilteredCommand {
    pub command_index: usize,
    pub score: i32,
    pub label_matches: Vec<MatchRange>,
}
```

Then:

- empty filter: populate all commands with no matches;
- non-empty filter: score every command label;
- keep only matches;
- sort by final score and tie-breakers;
- reset `selected_index` when filter changes.

### Rendering Highlights

Add a helper:

```rust
fn highlighted_label_job(
    label: &str,
    ranges: &[MatchRange],
    is_selected: bool,
) -> egui::text::LayoutJob
```

This helper should:

- split the original label into normal and highlighted spans;
- use existing selected/unselected colors;
- use a stronger color or underline/background for matched spans;
- avoid allocating more than necessary, but correctness first.

In `draw_command_row()`, use the stored match ranges for the current filtered row. If the app/action split remains, ranges need to be applied against the full label, or the label drawing should move to a single `LayoutJob` first.

Recommended first implementation: draw the whole label as one highlighted `LayoutJob`, then reintroduce separate app/action styling once the highlight path is stable.

## Action Metadata Plan

To support future priority, star, and tag behavior, treat each command/action as having metadata that is separate from the fuzzy match itself. The scorer should answer "how well does this query match this command?" Metadata should answer "how important or personally relevant is this command?"

Use these terms consistently:

- `priority`: one of `High`, `Normal`, or `Suppressed`.
- `starred`: a user preference that marks an action as personally important.
- `tags`: searchable labels such as `browser`, `tab`, `navigation`, `debug`, or `favorite`.

### TOML Shape

Keep app defaults, but allow action-level overrides:

```toml
[app]
id = "chrome"
name = "Chrome"
default_focus_state = "focused"
default_tags = ["browser"]

[actions.new_tab]
name = "New tab"
focus_state = "focused"
priority = "high"
tags = ["tab", "create"]
starred = false
cmd.windows = { mods = ["ctrl"], key = "T" }
```

Rules:

- `priority` belongs only on an action, never on the app.
- missing action priority resolves to `Normal`.
- TOML should use lowercase values: `high`, `normal`, `suppressed`.
- `Suppressed` is a hard lower priority bucket and must never sort above `Normal` within the same star bucket.
- `tags` on an action are merged with `app.default_tags`.
- `starred` should default to `false`.
- Unknown future metadata should not break old extensions unless it changes command execution behavior.

### Rust Model Shape

Add a metadata object instead of adding many loose fields to every struct:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandPriority {
    Suppressed,
    Normal,
    High,
}

#[derive(Debug, Clone)]
pub struct ActionMetadata {
    pub priority: CommandPriority,
    pub starred: bool,
    pub tags: Vec<String>,
}
```

Then include it in:

- `models::config::App`: `default_tags: Option<Vec<String>>`
- `models::config::Action`: `priority`, `tags`, `starred`
- `models::action::Action`: resolved metadata after applying app defaults
- `core::registry::UnitAction`: metadata passed to the UI
- `ui::ui_main::Command`: metadata used for filtering, sorting, and rendering

This keeps execution data, search data, and user preference data from becoming tangled.

### User Overrides

Extension TOML should describe default metadata. User choices like starred commands should eventually live outside extension files so updating an extension does not erase preferences.

Suggested future file:

```toml
[actions."chrome.new_tab"]
starred = true
priority = "high"
tags = ["daily"]
```

Use stable action keys for overrides:

```text
{app_id}.{action_id}
```

This means the registry should preserve the extension action id string, not only the current generated `u32` action counter. The current `ActionId = u32` is fine for runtime lookup, but user metadata should not depend on it because it can change when TOML action ordering changes.

### Sorting With Metadata

Recommended first sort key:

```text
star_bucket = Starred | Unstarred
priority_bucket = High | Normal | Suppressed
intra_bucket_score = fuzzy_score + focus_state_bonus + tag_match_bonus

sort_key = (
    star_bucket,
    priority_bucket,
    intra_bucket_score,
    is_prefix_match,
    compact_match_span,
    shorter_label,
    original_order
)
```

Initial values:

```text
Starred bucket: 1
Unstarred bucket: 0
High bucket: 2
Normal bucket: 1
Suppressed bucket: 0
focused bonus: +6
background bonus: +3
global bonus: +1
```

Starred commands should bypass unstarred commands, but not filtering. If the query does not match the command label, tags, alias, or shortcut search fields, the command should still be hidden. Inside the starred group, commands are still sorted by priority bucket, so a starred High command appears above a starred Normal command, and a starred Normal command appears above a starred Suppressed command.

### Searching Tags

Tags should become an additional search field after label matching works:

- label match gets full fuzzy score;
- tag match gets a smaller score, for example `tag_score * 0.6`;
- exact tag match gets a clear bonus;
- display tag matches only if they explain why the result appeared.

Example:

```text
query: debug
label: Chrome: Open Developer Tools
tags: ["debug", "devtools"]
```

This should rank well even if the label match is weak.

### UI Display

Keep metadata display quiet:

- show a small star marker for starred commands;
- show tags only when the query matches a tag or when a details row exists;
- avoid letting tags crowd out the command name and shortcut.

The first implementation can use a plain text marker such as `*` before adding icon assets.

## Priority Integration Plan

Current `Command` only has `label`, `shortcut_text`, and `action`. Extend it to include:

```rust
pub priority: CommandPriority,
pub focus_state: FocusState,
pub starred: bool,
pub tags: Vec<String>,
pub original_order: usize,
```

To populate this:

1. Store app/default priority and tags in `Application`.
2. Resolve action metadata when building each `Action`.
3. Store resolved metadata in `UnitAction`.
4. Pass metadata from `main.rs` into `Command`.
5. Sort by star bucket first, then priority bucket, then fuzzy result plus context/tag metadata.

This is a separate slice from the core scorer, so it can be implemented after fuzzy matching and highlighting.

## Testing Plan

Add unit tests in `src/core/search.rs`:

- sequential fuzzy match succeeds: `nt` -> `Chrome: New tab`;
- wrong order fails: `tn` should not match `Chrome: New tab` as a strong ordered match;
- prefix beats scattered: `new` in `New tab` beats `nwt` in `New window tab`;
- separator bonus: `nt` should prefer `New tab` over `Notification tracker` when scores are close;
- camel-case bonus: `dt` matches `DeveloperTools` at word/camel boundaries;
- multi-piece query requires all pieces: `chrome tab` matches `Chrome: New tab`;
- quoted query requires contiguous substring;
- ranges merge consecutive indices into one range;
- Unicode-safe byte ranges do not panic when labels contain non-ASCII text.
- starred commands sort above unstarred commands but still require a search match;
- action-level priority overrides app-level priority;
- within the same star bucket, a Suppressed command never sorts above a Normal command, even with a better fuzzy score;
- a starred Suppressed command can sort above an unstarred Normal command because star bucket wins first;
- action tags merge with app default tags;
- stable action keys use extension ids rather than runtime `u32` ids.

Add UI-level tests only if the app gains a test harness. For now, keep UI helper functions pure enough to unit test range splitting without launching egui.

## Implementation Phases

### Phase 1: Core Scorer

- Replace `do_score_contains()` and remove debug prints.
- Add `PreparedQuery`, `QueryPiece`, `MatchRange`, and richer `MatchResult`.
- Implement single-piece fuzzy DP scorer.
- Add range normalization.
- Add unit tests for core scoring.

### Phase 2: UI Filtering And Highlights

- Store scored filtered rows instead of only indices.
- Use scorer ranges in `draw_command_row()`.
- Render highlighted result labels with `LayoutJob`.
- Keep keyboard navigation and Enter execution behavior unchanged.

### Phase 3: Priority Sorting

- Thread priority from TOML config into registry actions.
- Add priority/focus/star/tag metadata to `Command`.
- Sort by star bucket first, then priority bucket, then fuzzy score and metadata inside the bucket.
- Add tests for sort order where possible.

### Phase 4: Stars And Tags

- Add action-level metadata to config parsing.
- Add app-level default tags.
- Preserve stable app/action ids for user overrides.
- Add a quiet UI marker for starred commands.
- Add tag search as a secondary search field.

### Phase 5: Query Polish

- Add multi-piece query support.
- Add quoted exact/contiguous query support.
- Add optional scorer cache keyed by `(query, command_index)` if profiling shows it matters.

## Questions / Decisions

- Should highlighted text appear only in result rows, or also inside the search input itself? I assume result rows for the first implementation.
- Should starred state come from extension TOML only, user override TOML only, or both? I recommend both, with user overrides winning.
- Should tags be visible all the time or only when they explain a match? I recommend only when they explain a match.
- Should shortcuts be searchable too? Example: typing `ctrl t` finds `Chrome: New tab`. This is useful, but I would keep it phase 2/3 after label search is solid.
- Should recent/frequent command usage affect ranking? This is valuable but needs persistence, so it should be a later design.

## Acceptance Criteria

- Filtering is fuzzy, not only simple contains or first sequential match.
- Matches are deterministic and stable.
- The result list visibly highlights matched characters/ranges while typing.
- Commands are sorted by star bucket first, then priority bucket, then fuzzy quality, focus state, and tags.
- Starred commands show above unstarred commands.
- Within the same star bucket, Suppressed commands never show above Normal commands.
- Empty query remains fast and shows commands in default order.
- Core scorer has unit coverage before UI integration.
