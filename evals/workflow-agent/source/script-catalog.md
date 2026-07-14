# Script and Narrative Version Catalog

## Scope and authority

This catalog describes narrative evidence in the source snapshot identified by SHA-256 `45b5a147b658dd6cce68d21eabb33508801bff29757ecc7de39e06443093fe74`.

Source locators use zero-based `nodeList` positions so an authorized reviewer can reproduce each observation without committing raw node identifiers, text, prompts, URLs, or media. A locator proves that an artifact existed in the observed snapshot. It does not prove that the artifact was approved.

No master script or Gold lineage is selected here. Canvas position, timestamps, duplicated text, downstream use, and apparent polish are insufficient approval evidence.

## Stable story facts shared across major versions

The major narrative versions consistently describe:

- a guardian figure defending an apparently immense frontier from a mechanical threat;
- a scout, two drum-associated attendants, and a tiger companion in the fantasy layer;
- a battle or charge that transitions into a warm domestic reality;
- a reveal that the fantasy characters and locations correspond to household objects and toys;
- an ending in which damage to the guardian figure is repaired by a child and reinterpreted as honorable protection.

These are candidate canonical facts because they recur across versions. Their exact wording, visual design, timing, and implementation remain version-dependent.

## Major narrative and shot versions

| ID | Scope | Observed size | Source locators | Observable relationship | Authority |
| --- | --- | ---: | --- | --- | --- |
| `script-base-001` | Complete screenplay | About 3 minutes; four scenes | `nodeList[11].data.content`, duplicate at `nodeList[97].data.content` | `nodeList[97]` directly feeds the initial shot script at `nodeList[21]` | Unresolved |
| `shots-initial-001` | Complete shot treatment | 16 numbered shots in four acts | `nodeList[21].data.content`, duplicate at `nodeList[48].data.content` | The duplicate directly feeds the later replacement at `nodeList[455]`; three story-table nodes expose 16 shot columns | Unresolved |
| `shots-revised-029` | Complete revised shot script | 29 numbered shots; prologue plus four acts | `nodeList[118].data.content` | No observed direct graph edge establishes adoption by a production branch | Unresolved |
| `shots-composite-017` | Complete composite candidate | Shots 1–6 inherited from `shots-initial-001`; replacement shots 7–17 | `nodeList[48].data.content`, `nodeList[455].data.content` | A direct edge connects the 16-shot duplicate to the replacement; the replacement explicitly changes acts two through four | Unresolved |
| `shots-reality-023` | Partial late-story branch | 23 numbered shots covering the reality reveal and ending | `nodeList[254].data.content` | Receives the same late-story brief as sibling variants and directly feeds `shots-reality-028` | Unresolved |
| `shots-reality-028` | Partial late-story optimization | 28 numbered shots covering the reality reveal and ending | `nodeList[250].data.content` | Receives both the late-story brief and `shots-reality-023`; it describes itself as an optimization | Unresolved |
| `battle-sequence-008a` | Partial battle branch | 8 shots over 30 seconds | `nodeList[31].data.content` | Directly downstream of `shots-composite-017`; ends at the moment before collision | Unresolved |
| `battle-sequence-008b` | Partial battle branch with stricter technical constraints | 8 shots over 30 seconds | `nodeList[255].data.content` | Feeds two separate music-design nodes; content substantially overlaps `battle-sequence-008a` | Unresolved |

## Structured storyboard evidence

Three `video_story_resource` nodes exist at `nodeList[51]`, `nodeList[106]`, and `nodeList[107]`.

- Each exposes 16 shot columns.
- Their row counts are two, two, and one respectively.
- Each has one incoming edge and no outgoing edge in the saved graph.
- The snapshot does not prove whether the three tables are alternatives, split views, or superseded copies.

The storyboard tables corroborate a 16-shot structure but do not establish that `shots-initial-001` is the approved master.

## Partial narrative, production, and companion artifacts

The following evidence is cataloged so that no narrative-related node silently disappears. These artifacts are not promoted to complete script versions.

| Source locator | Classification | Observable content shape | Relationship or limitation |
| --- | --- | --- | --- |
| `nodeList[1]` | Visual prompt fragment | One creature-scale image brief | No script coverage or approval evidence |
| `nodeList[9]` | Micro-sequence | Three timed beats over 10 seconds | Feeds three downstream nodes but is not a full act |
| `nodeList[25]` | Visual asset brief | One damaged-banner design | Asset-specific, not a narrative version |
| `nodeList[31]` | Battle shot branch | Eight numbered shots | Cataloged above as `battle-sequence-008a` |
| `nodeList[51]` | Story table | Sixteen shot columns, two rows | Structured evidence only |
| `nodeList[59]` | Text fragment | Short production instruction | Insufficient narrative coverage |
| `nodeList[98]` | Scene-design treatment | Art direction for the third and fourth acts | Directly downstream of the 16-shot duplicate; includes generation-ready boundary material |
| `nodeList[106]` | Story table | Sixteen shot columns, two rows | Structured evidence only |
| `nodeList[107]` | Story table | Sixteen shot columns, one row | Structured evidence only |
| `nodeList[138]` | Shot fragment | Three numbered shots | Local production fragment |
| `nodeList[143]` | Character-reaction staging | Multi-character reaction and motion notes | Directly downstream of another production node; not a shot-list master |
| `nodeList[242]` | Shot fragment | Three numbered shots | Local production fragment |
| `nodeList[245]` | Ending branch | Eleven numbered ending shots | Directly feeds an ending-music design node |
| `nodeList[246]` | Music design | Timed score structure | Directly downstream of `battle-sequence-008b`; audio companion, not narrative Gold |
| `nodeList[247]` | Ending prompt set | Seven numbered shots | Condensed late-story prompt set |
| `nodeList[248]` | Ending prompt set | Eight numbered shots | Receives the late-story brief plus five reference-producing inputs |
| `nodeList[249]` | Late-story prose revision | Third and fourth acts with dialogue and inner monologue | Directly feeds the 8-, 23-, and 28-shot late-story variants |
| `nodeList[250]` | Late-story shot optimization | Twenty-eight numbered shots | Cataloged above as `shots-reality-028` |
| `nodeList[251]` | Music design | Timed battle score | Directly downstream of `battle-sequence-008b` |
| `nodeList[252]` | Empty generation artifact | No saved content | Must remain unresolved; no narrative evidence can be recovered from the snapshot |
| `nodeList[253]` | Music design | Timed ending score | Directly downstream of the eleven-shot ending branch |
| `nodeList[254]` | Late-story shot branch | Twenty-three numbered shots | Cataloged above as `shots-reality-023` |
| `nodeList[255]` | Battle source branch | Eight numbered shots plus technical constraints | Cataloged above as `battle-sequence-008b` |
| `nodeList[455]` | Replacement shot script | Eleven numbered replacement shots, numbered 7–17 | Combines with inherited shots 1–6 to form `shots-composite-017` |

Together with the five complete screenplay/shot artifacts and their duplicates at `nodeList[11]`, `nodeList[21]`, `nodeList[48]`, `nodeList[97]`, and `nodeList[118]`, this table accounts for all 29 text/story nodes identified by the workflow inventory.

## Explicit revision deltas

The replacement at `nodeList[455]` states the following changes relative to the initial 16-shot treatment:

- replace a black tiger with a natural yellow-and-black tiger and give it a stable name;
- remove sacrificial or death-oriented framing from the second act;
- emphasize a deliberate heroic charge rather than a fatal leap;
- make the fantasy-to-reality mapping of guardian figure and tiger explicit;
- add first-person pull-back and room-reveal camera language;
- keep the guardian figure and tiger together through the ending.

The late-story branches add a separate production constraint:

- the grandfather and child must not show identifiable faces;
- hands, feet, backs, silhouettes, partial bodies, and shadows are preferred;
- the restriction is motivated by generation stability and visual style, not by the base screenplay.

## Conflicts requiring authority decisions

| Conflict | Competing evidence | Why it remains unresolved |
| --- | --- | --- |
| Tiger design | Black tiger in the base and early shot scripts; natural yellow-and-black tiger in later revisions | A later explicit change exists, but no user approval or selected final output is recorded |
| Battle meaning | Sacrificial last stand and fatal-leap language versus heroic breach and continued guardianship | The replacement explicitly rejects the earlier emotional framing, but approval is unproven |
| Battle outcome | Direct strike followed by collapse and reality transition versus an eight-shot branch that cuts to black before collision | The branches serve different durations and may be alternatives rather than revisions |
| Human visibility | Early versions show faces and facial reactions; later branches prohibit identifiable faces | The no-face rule is clear within its branch but is not proven project-wide |
| Repair symbol | One complete revision uses a heart-shaped repair; later branches use a five-point star | No selected final media or approval record resolves the symbol |
| Shot count | Complete candidates contain 16, 17, or 29 shots; partial branches contain 23, 28, and 8 shots | Partial branch counts cannot be compared directly with complete-project counts |
| Camera specification | Different artifacts cite different camera bodies, formats, frame rates, and lenses | These may be prompt-level experiments rather than authoritative production constraints |
| Music duration | Battle and ending score artifacts use durations that do not always match their associated visual branch | The source does not expose an approved timing lock or final mix |

## Missing evidence

- No explicit approval flag, selected-version record, or final-master identifier was observed.
- No reliable mapping proves that every generated media branch belongs to one particular script version.
- Duplicate content does not reveal whether the duplicate is a backup, a fork, or the selected copy.
- The snapshot does not provide a complete edit decision list or final timeline.
- The empty text-generation artifact at `nodeList[252]` cannot be reconstructed.
- Some late-story branches contain richer privacy and generation constraints than the complete scripts, but their project-wide applicability is unknown.

## Catalog conclusion

The source supports multiple production lineages, not one safely inferable master. The Ledger Set must therefore retain the complete 29-shot, 17-shot composite, initial 16-shot, late-story 23/28-shot, and eight-shot battle candidates until observable evidence or user review establishes authority. Partial branches may inform group-level production requirements, but they must not silently replace full-script coverage.
