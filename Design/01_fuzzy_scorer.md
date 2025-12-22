Our scorer will be based on https://github.com/microsoft/vscode/blob/main/src/vs/base/common/fuzzyScorer.ts



# 1. The Core Filter: Sequential Matching
The absolute rule in VS Code's fuzzy search is that characters must appear in the target string in the same order they were typed, but they don't have to be adjacent.

Query: gitps

Match: [Git]: [P]u[s]h (Valid: g...i...t...p...s)

No Match: [S]te[p] [I]n[t]o (Invalid: order is wrong)

# 2. The Scoring Heuristics
Once the filter identifies potential matches, VS Code ranks them using a weighted scoring system. You can replicate this in your Rust project by assigning "points" to each match:

### Word Starts
High Bonus,"Matches like ""Save File"" for query sf are highly intentional."
### CamelCase
- Medium Bonus
- "Matches capital letters (e.g., fsm for ""FiniteStateMachine"")."
### Consecutive
- Small Bonus
- """App"" matching ""Application"" is better than ""Aup****port""."
### Separator
- Small Bonus
- "Characters immediately following a /, ., or _."
### Recency
- Multiplier
- Items you used 2 minutes ago are boosted to the top.