# Unresolved Ledger-Set Evidence

## Gate status

The five provisional Ledger Groups cover all 29 shots and 180 seconds of the working candidate, but no group has approved lineage authority. This document lists the decisions required at the Ledger-Set Approval Gate. It does not ask the reviewer to approve raw prompts, private media, provider identifiers, or canvas-derived assumptions.

## Decision principles

- A recurring story fact may be treated as a candidate canonical fact without selecting one implementation.
- A direct graph edge proves lineage or consumption, not approval.
- A replacement can supersede material inside one lineage without globally rejecting a complete competing version.
- Stale or retry metadata proves workflow state, not creative failure or rejection.
- Ordered references prove order for the observed consumer only; they do not prove project-wide ownership.
- Any unresolved group remains in the benchmark and is scored as unresolved unless the user approves a decision or explicitly approves exclusion.

## Required decisions

| Decision ID | Decision | Groups | Competing evidence | Approval needed |
| --- | --- | --- | --- | --- |
| `decision-master-version` | Master version | All | Complete 16-shot, 17-shot composite, and 29-shot candidates plus the base screenplay | Select the master semantics and identify which other complete versions remain acceptable alternatives |
| `decision-first-act-split` | First-act partition | 01-02 | Exact prologue-plus-act timing versus a semantically cleaner split after threat assessment with no per-shot time boundary | Approve the exact five-group baseline or provide timing for the semantic split |
| `decision-tiger-design` | Tiger visual design | 02-04 | Black tiger versus natural yellow-and-black tiger | Approve the visual design |
| `decision-tiger-name` | Tiger name | 02-03 | A stable name appears only in a replacement branch | Approve a name or make naming non-binding |
| `decision-tiger-mapping` | Tiger reality mapping | 03-04 | Competing fantasy designs require a consistent household-object mapping | Approve the cross-reality mapping |
| `decision-battle-framing` | Battle framing | 02-03 | Sacrificial last stand versus deliberate heroic breach | Approve the emotional meaning that must survive professional alternatives |
| `decision-battle-outcome` | Battle outcome | 03-04 | Shown collision and disabled threat versus cut before impact | Approve the event state handed into the reality reveal |
| `decision-human-visibility` | Human visibility | 04-05 | Face-visible complete candidates versus no-identifiable-face late-story branches | Approve whether the restriction is project-wide, group-local, or rejected |
| `decision-repair-symbol` | Repair symbol | 05 | Heart in the 29-shot candidate versus five-point star in later ending branches | Approve the symbol or approve a semantic tolerance that permits alternatives |
| `decision-g01-camera-body` | Threat camera body | 01 | Competing body specifications | Approve one or make it non-binding |
| `decision-g01-camera-format` | Threat capture format | 01 | Competing format specifications | Approve one or make it non-binding |
| `decision-g01-frame-rate` | Threat frame rate | 01 | Competing frame-rate specifications | Approve one or make it non-binding |
| `decision-g01-lens` | Threat lens contract | 01 | Competing lens specifications | Approve one or make it non-binding |
| `decision-g02-camera-body` | Mobilization camera body | 02 | Competing body specifications | Approve one or make it non-binding |
| `decision-g02-camera-format` | Mobilization capture format | 02 | Competing format specifications | Approve one or make it non-binding |
| `decision-g02-frame-rate` | Mobilization frame rate | 02 | Competing frame-rate specifications | Approve one or make it non-binding |
| `decision-g02-lens` | Mobilization lens contract | 02 | Competing lens specifications | Approve one or make it non-binding |
| `decision-g03-camera-body` | Charge camera body | 03 | Competing body specifications | Approve one or make it non-binding |
| `decision-g03-camera-format` | Charge capture format | 03 | Competing format specifications | Approve one or make it non-binding |
| `decision-g03-frame-rate` | Charge frame rate | 03 | Competing normal and slow-motion specifications | Approve the binding rates |
| `decision-g03-lens` | Charge lens contract | 03 | Competing lens specifications | Approve one or make it non-binding |
| `decision-g04-camera-body` | Reveal camera body | 04 | Competing body specifications | Approve one or make it non-binding |
| `decision-g04-camera-format` | Reveal capture format | 04 | Competing format specifications | Approve one or make it non-binding |
| `decision-g04-frame-rate` | Reveal frame rate | 04 | Competing frame-rate specifications | Approve one or make it non-binding |
| `decision-g04-lens` | Reveal lens contract | 04 | Competing lens specifications | Approve one or make it non-binding |
| `decision-g05-camera-body` | Restoration camera body | 05 | Competing body specifications | Approve one or make it non-binding |
| `decision-g05-camera-format` | Restoration capture format | 05 | Competing format specifications | Approve one or make it non-binding |
| `decision-g05-frame-rate` | Restoration frame rate | 05 | Competing frame-rate specifications | Approve one or make it non-binding |
| `decision-g05-lens` | Restoration lens contract | 05 | Competing lens specifications | Approve one or make it non-binding |
| `decision-battle-music-timing` | Battle music timing | 03 | Music durations do not consistently match battle branches | Approve a timing lock |
| `decision-ending-music-timing` | Ending music timing | 05 | Music duration does not consistently match ending branches | Approve a timing lock |
| `decision-audio-motif-contract` | Cross-group audio motifs | 01-05 | Machine, drum, domestic, and restoration motifs vary by branch | Approve required motif handoffs |
| `decision-design-only-audio-evidence` | Design-only audio evidence | 03-05 | Music-design text exists without rendered audio | Decide whether it may inform Gold semantics |
| `decision-g01-image-join` | Threat image lineage | 01 | No direct image join from the working candidate | Approve a source-backed mapping or retain it as unresolved |
| `decision-g01-video-join` | Threat video lineage | 01 | No direct video join from the working candidate | Approve a source-backed mapping or retain it as unresolved |
| `decision-g01-audio-join` | Threat audio lineage | 01 | No direct audio join from the working candidate | Approve a source-backed mapping or retain it as unresolved |
| `decision-g02-image-join` | Mobilization image lineage | 02 | No direct image join from the working candidate | Approve a source-backed mapping or retain it as unresolved |
| `decision-g02-video-join` | Mobilization video lineage | 02 | No direct video join from the working candidate | Approve a source-backed mapping or retain it as unresolved |
| `decision-g02-audio-join` | Mobilization audio lineage | 02 | No direct audio join from the working candidate | Approve a source-backed mapping or retain it as unresolved |
| `decision-g03-image-join` | Charge image lineage | 03 | No direct image join from the working candidate or battle branches | Approve a source-backed mapping or retain it as unresolved |
| `decision-g03-video-join` | Charge video lineage | 03 | No direct video join from the working candidate or battle branches | Approve a source-backed mapping or retain it as unresolved |
| `decision-g03-audio-join` | Charge audio lineage | 03 | Music designs exist without a rendered-audio join | Approve a rendered mapping or retain design-only evidence as unresolved |
| `decision-g04-image-join` | Reveal image lineage | 04 | Six ordered comparative references have unresolved group ownership | Approve, reject, or retain the ordered set as experimental |
| `decision-g04-video-join` | Reveal video lineage | 04 | No direct group-specific video join | Approve a source-backed mapping or retain it as unresolved |
| `decision-g04-audio-join` | Reveal audio lineage | 04 | Dialogue, ambience, and object sounds lack approved rendered evidence | Approve a source-backed mapping or retain it as unresolved |
| `decision-g05-image-join` | Restoration image lineage | 05 | Six ordered comparative references have unresolved group ownership | Approve, reject, or retain the ordered set as experimental |
| `decision-g05-video-join` | Restoration video lineage | 05 | No direct group-specific video join | Approve a source-backed mapping or retain it as unresolved |
| `decision-g05-audio-join` | Restoration audio lineage | 05 | Ending music design exists without an approved rendered mix | Approve a rendered mapping or retain design-only evidence as unresolved |
| `decision-opening-structure` | Opening narrative structure | 01 | Competing complete structures | Approve the narrative opening structure |
| `decision-typography` | Opening typography | 01 | Typography exists in the working candidate but has no cross-version or production authority | Approve it as binding, tolerate alternatives, or classify it as experimental |
| `decision-battle-branch-a` | First eight-shot battle branch | 03 | Partial branch ends before collision | Classify it as acceptable, experimental, superseded, or unresolved |
| `decision-battle-branch-b` | Second eight-shot battle branch | 03 | Partial branch adds stricter technical constraints | Classify it as acceptable, experimental, superseded, or unresolved |
| `decision-battle-music-design-a` | First battle music design | 03 | Design text is downstream of a partial battle branch | Classify it and decide whether it may inform Gold |
| `decision-battle-music-design-b` | Second battle music design | 03 | Competing design text is downstream of the same branch | Classify it and decide whether it may inform Gold |
| `decision-late-story-23` | 23-shot branch disposition | 04-05 | Partial late-story branch contributes to another branch but is not proven replaced | Classify its semantics as acceptable, experimental, superseded, or unresolved |
| `decision-late-story-28` | 28-shot branch disposition | 04-05 | Partial optimization branch lacks project-wide approval | Classify its semantics as acceptable, experimental, or unresolved |
| `decision-ending-prompt-a` | First ending prompt set | 05 | Condensed prompt set lacks selection evidence | Classify it as approved, acceptable, experimental, or unresolved |
| `decision-ending-prompt-b-g04` | Second ending prompt set for reveal | 04 | Prompt branch has ordered references but unresolved reveal ownership | Classify the prompt branch for group 04 |
| `decision-ending-prompt-b-g05` | Second ending prompt set for restoration | 05 | Prompt branch has ordered references but unresolved restoration ownership | Classify the prompt branch for group 05 |
| `decision-empty-artifact` | Empty generation artifact | 05 | The saved artifact has no recoverable content | Retain as missing evidence, exclude with approval, or replace with external authority |
| `decision-restoration-repair` | Repair visual lineage | 05 | Repair action and symbol have no approved rendered lineage | Identify approved evidence or retain it as unresolved |
| `decision-restoration-placement` | Final placement lineage | 05 | Guardian placement and child interaction lack approved rendered lineage | Identify approved evidence or retain it as unresolved |
| `decision-restoration-voiceover` | Final voice-over lineage | 05 | Inner monologue and renewed-purpose speech lack approved rendered audio | Identify approved evidence or retain it as unresolved |
| `decision-restoration-score` | Final score lineage | 05 | Timed design exists without an approved mix | Identify approved evidence or retain it as unresolved |
| `decision-restoration-title` | Final title lineage | 05 | Final image, fade, and title have no approved rendered lineage | Identify approved evidence or retain it as unresolved |
| `decision-g02-g03-spatial-handoff` | Mobilization-to-charge spatial handoff | 02-03 | Guardian, tiger, weapon, and frontier state must cross the boundary without an approved implementation | Approve the required spatial and asset state |
| `decision-g03-g04-spatial-handoff` | Charge-to-reveal spatial handoff | 03-04 | Guardian-tiger positions and damage must support object mapping | Approve the required spatial state for both battle outcomes |

## Group-specific unresolved evidence

### Ledger 01: Threat

- `decision-opening-structure`: Choose the authoritative opening narrative structure.
- `decision-g01-image-join`, `decision-g01-video-join`, and `decision-g01-audio-join`: Identify approved threat, frontier, guardian-handoff, and machine-motif production branches.
- `decision-g01-camera-body`, `decision-g01-camera-format`, `decision-g01-frame-rate`, `decision-g01-lens`, and `decision-typography`: Decide which camera and typography constraints are binding.

### Ledger 02: Mobilization

- `decision-tiger-design`, `decision-tiger-name`, and `decision-battle-framing`: Resolve tiger identity and the framing carried into the charge.
- `decision-first-act-split`: Decide whether the first timed act should split after threat assessment once timing is available.
- `decision-g02-image-join`, `decision-g02-video-join`, and `decision-g02-audio-join`: Identify approved cast, frontier, dialogue, drum, and tiger-arrival references.
- `decision-g02-g03-spatial-handoff`: Approve the weapon, frontier, guardian, and tiger state handed into the charge.
- `decision-g02-camera-body`, `decision-g02-camera-format`, `decision-g02-frame-rate`, and `decision-g02-lens`: Resolve mobilization camera constraints.

### Ledger 03: Charge

- `decision-battle-outcome`: Resolve shown impact versus pre-impact cut and the resulting threat state.
- `decision-battle-framing`: Resolve heroic versus sacrificial meaning.
- `decision-battle-branch-a`, `decision-battle-branch-b`, `decision-battle-music-design-a`, and `decision-battle-music-design-b`: Classify the battle alternatives and music designs independently.
- `decision-g03-image-join`, `decision-g03-video-join`, `decision-g03-audio-join`, `decision-battle-music-timing`, and `decision-design-only-audio-evidence`: Identify rendered media evidence and decide how design-only text may inform Gold.
- `decision-g03-g04-spatial-handoff`: Approve the guardian-tiger position and damage state handed into the reveal.
- `decision-g03-camera-body`, `decision-g03-camera-format`, `decision-g03-frame-rate`, and `decision-g03-lens`: Resolve charge camera constraints.

### Ledger 04: Reveal

- `decision-human-visibility`, `decision-tiger-design`, and `decision-tiger-mapping`: Resolve the no-face policy and tiger object mapping.
- `decision-late-story-23` and `decision-late-story-28`: Decide whether either partial lineage contributes approved semantics.
- `decision-g04-image-join` and `decision-ending-prompt-b-g04`: Decide whether the prompt branch and six ordered image references are approved for this group or only experimental.
- `decision-g04-video-join` and `decision-g04-audio-join`: Identify approved reality-transition, mapping, dialogue, ambience, and object-sound artifacts.
- `decision-g04-camera-body`, `decision-g04-camera-format`, `decision-g04-frame-rate`, and `decision-g04-lens`: Resolve reveal camera constraints.

### Ledger 05: Restoration

- `decision-repair-symbol`: Resolve heart versus star repair symbolism.
- `decision-late-story-23`, `decision-late-story-28`, `decision-ending-prompt-a`, `decision-ending-prompt-b-g05`, `decision-human-visibility`, and `decision-ending-music-timing`: Resolve ending prompt and shot lineage, human visibility, and ending-music timing.
- `decision-empty-artifact`: Decide how to treat the empty generation artifact; its content cannot be reconstructed.
- `decision-restoration-repair`, `decision-restoration-placement`, `decision-restoration-voiceover`, `decision-restoration-score`, and `decision-restoration-title`: Identify approved final artifacts.
- `decision-g05-image-join`, `decision-g05-video-join`, and `decision-g05-audio-join`: Record the modality-level production lineage decisions.
- `decision-g05-camera-body`, `decision-g05-camera-format`, `decision-g05-frame-rate`, and `decision-g05-lens`: Resolve restoration camera constraints.
- `decision-audio-motif-contract` and `decision-design-only-audio-evidence`: Resolve cross-group motif continuity and the authority of design-only audio evidence.

## Explicit non-decisions

The current evidence does not justify any approved reference, acceptable alternative, project-wide supersession, or final media selection. Those lists remain empty in `lineage-authority.json` by design. Local replacement records are retained only inside the lineages where direct relationships exist.

## Approval outcomes to record

For every decision above, record one of:

1. approved canonical requirement;
2. approved reference;
3. acceptable alternative with measurable tolerance;
4. experiment;
5. superseded within a named scope;
6. unresolved scored case;
7. explicit exclusion with rationale and confirmation that script and modality coverage remain complete.

After decisions are recorded, rerun shot/time coverage, continuity, modality, ordered-reference, and authority-consistency checks before Task 9 begins.
