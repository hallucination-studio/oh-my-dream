# Workflow Assistant Benchmark Specification

## 1. Script Ledger Group boundary rules

### 1.1 Purpose

A Script Ledger Group is the smallest independently reviewable production unit that preserves a coherent script beat and can be revised or rerun without unnecessarily invalidating approved work in adjacent groups.

Groups partition an approved script. They are not inferred from the workflow canvas and they are not separate projects.

The final ordered group set must satisfy all of the following invariants:

1. **Exhaustive coverage:** every approved script beat and every point in the approved runtime belongs to one group.
2. **No overlap:** no script beat or time interval belongs to more than one group.
3. **Stable order:** group order follows narrative time, including flashbacks or parallel action as explicitly authored by the script.
4. **Explicit handoff:** every non-initial group declares the continuity state it inherits; every non-final group declares the state it hands forward.
5. **Bounded production scope:** the media operations, review work, and smallest sufficient rerun for one group fit within a declared execution budget.
6. **Source-backed boundaries:** every boundary cites narrative or production evidence from the approved lineage.

If the master script, timing, or lineage authority is unresolved, the partition remains provisional and cannot pass the Ledger-Set Approval Gate.

### 1.2 Required boundary evidence

A proposed boundary is accepted only when at least one primary reason and all required safeguards are present.

Primary reasons:

- **Narrative transition:** a scene, sequence, dramatic objective, point of view, location, time, or reality layer changes.
- **Production transition:** the next beat requires a materially different asset set, reference package, modality plan, provider operation, or assembly strategy.
- **Revision boundary:** feedback can target the preceding unit without semantically rewriting the following unit.
- **Execution boundary:** the preceding unit has a natural terminal artifact or checkpoint that can be reviewed before cost is committed downstream.

Required safeguards:

- the boundary does not split an indivisible action, dialogue exchange, reveal, or transition;
- incoming identity, wardrobe, props, geography, lighting, motion, dialogue, audio motif, and timing state can be stated explicitly;
- outgoing continuity is sufficient for the next group to proceed without rereading hidden source material;
- the boundary does not create an orphaned media branch or duplicate a shared dependency without justification;
- the group has a declared smallest sufficient rerun after a local failure or revision.

### 1.3 Invalid boundary evidence

The following signals never justify a boundary by themselves:

- canvas coordinates, visual proximity, lanes, frames, or group-box placement;
- node creation or update timestamps;
- labels, numbering, or repeated names without matching script coverage;
- node degree, downstream popularity, or the number of generated variants;
- an attractive or memorable shot;
- a provider batch size or arbitrary fixed shot count;
- a desire to omit an expensive, ambiguous, or low-quality section.

These signals may help locate evidence after a script boundary is established, but they cannot define the boundary.

### 1.4 Group sizing rules

There is no universal number of shots per group. Size is determined by the conjunction of narrative cohesion and bounded production work.

A group is too large when any of the following is true:

- a single feedback request would commonly target only one internal sub-sequence;
- two or more independent asset/reference packages can be revised without affecting each other;
- the group cannot declare one bounded candidate, cost, latency, retry, and smallest-rerun policy;
- checkpoint evaluation cannot identify which internal beat caused a semantic or media failure;
- continuation evaluation would hide an accumulated error until much later in the project.

A group is too small when any of the following is true:

- it contains only a camera cut while the dramatic objective and production package remain unchanged;
- it separates setup from payoff, action from immediate consequence, or dialogue from the response needed to interpret it;
- it requires repeating the same identity/reference setup with no independent revision benefit;
- it produces no independently reviewable artifact or continuity handoff;
- its only justification is a node, prompt, or provider-call boundary.

### 1.5 Boundary decision record

Every accepted boundary must record:

| Field | Requirement |
| --- | --- |
| `boundary_id` | Stable benchmark identifier |
| `before_group` | Group ending at the boundary |
| `after_group` | Group beginning at the boundary |
| `script_locator` | Approved script beat and time boundary |
| `primary_reason` | Narrative, production, revision, or execution transition |
| `evidence` | Source-backed facts supporting the reason |
| `incoming_continuity` | State the next group inherits |
| `outgoing_continuity` | State the preceding group commits |
| `shared_dependencies` | Assets or references intentionally reused across the boundary |
| `revision_isolation` | Why a local change can stop at this boundary |
| `smallest_rerun` | Minimal subgraph after failure or feedback |
| `unresolved` | Missing authority or evidence that blocks approval |

### 1.6 Partition validation algorithm

For a candidate master script:

1. Normalize it into ordered atomic beats with explicit time intervals when available.
2. Assign every atomic beat to exactly one candidate group.
3. Reject gaps, overlaps, reversed order, and unbounded intervals.
4. Evaluate every proposed boundary against the required evidence and safeguards.
5. Merge groups that are separated only by invalid evidence or that split an indivisible beat.
6. Split groups that contain independent revision or execution scopes.
7. Write incoming and outgoing continuity for every adjacent pair.
8. Map shared assets, required modalities, terminal artifacts, and downstream consumers.
9. Define the smallest sufficient rerun and cost/retry bounds for every group.
10. Preserve alternative valid partitions and unresolved boundaries for human review.

The selected partition is not Gold until the user approves the master script, boundaries, lineage authority, continuity, and modality matrix.

## 2. Provisional partition comparison

The following comparison applies the rules to the stable story facts in the source catalog. It tests the rules; it does not approve a master script or create the Script Ledger Index.

### 2.1 Candidate A: five production-sized groups

| Group | Abstract beat range | Decision | Rationale |
| --- | --- | --- | --- |
| `A1-threat` | Ominous setup through confirmation of the approaching mechanical threat | Provisionally acceptable | The threat reveal forms one objective and can terminate in an independently reviewable environment/threat package |
| `A2-mobilization` | Scout warning through guardian, attendants, and tiger committing to action | Provisionally acceptable | Character introductions, dialogue, identity references, and mobilization share one continuity package and one dramatic decision |
| `A3-charge` | Structural attack through charge, impact or pre-impact cut, collapse, and loss of fantasy consciousness | Provisionally acceptable with unresolved ending | This is one action-consequence unit; splitting leap from immediate outcome would separate setup from payoff, but competing versions disagree on whether impact is shown |
| `A4-reveal` | Warm reality transition through mapping fantasy elements to household objects | Provisionally acceptable | Reality layer, lighting, scale, asset semantics, and soundscape change together; the reveal must remain cohesive to preserve its meaning |
| `A5-restoration` | Damage recognition through child repair, renewed guardianship, and final image | Provisionally acceptable | Repair and emotional reinterpretation form one payoff with a clear final artifact and local revision scope |

Accepted provisional boundaries:

1. **`A1 -> A2`: threat understood to response begun.** The dramatic objective changes from discovering danger to choosing a response. Threat appearance and environment state hand forward unchanged.
2. **`A2 -> A3`: commitment to physical execution.** Character and asset setup completes before the destructive action begins. Identity, equipment, positions, wind, lighting, and audio tension hand forward.
3. **`A3 -> A4`: fantasy consciousness to domestic reality.** This is the strongest boundary: reality layer, scale, location semantics, palette, lighting, ambience, and object interpretation change together. The damaged guardian and tiger positions hand forward.
4. **`A4 -> A5`: truth established to meaning restored.** The object mapping and family context are known; the remaining objective is repair and emotional resolution. Damage state, object placement, and relationship state hand forward.

Candidate A is the strongest current partition because each group has a distinct objective and production package while preserving setup/payoff pairs. It remains provisional because the approved script version, exact timing, battle outcome, tiger design, human-visibility policy, and repair symbol are unresolved.

### 2.2 Candidate B: three act-sized groups

| Group | Abstract beat range | Decision | Failure |
| --- | --- | --- | --- |
| `B1-fantasy-setup` | Threat and mobilization | Reject without further split | Combines discovery and response, which have separable feedback and production scopes |
| `B2-battle` | Attack through collapse | Potentially acceptable | Equivalent to Candidate A's action group if one battle lineage is approved |
| `B3-reality` | Reveal through final restoration | Reject without further split | Combines a scale/location/asset reveal with a separate repair and emotional-resolution objective |

Candidate B is too coarse. A failure in object mapping, privacy-safe human staging, repair action, or final emotional beat would force a broad late-story rerun and obscure the failed contract.

### 2.3 Candidate C: one group per shot

Candidate C is rejected as systematically over-fragmented.

- Most cuts do not create a new dramatic objective or production package.
- Identity and environment references would be repeated across many tiny tasks.
- Setup and payoff pairs would be separated.
- Individual shots often lack an independent terminal artifact meaningful at project level.
- Continuation scoring would over-weight local camera compliance and under-weight sequence continuity.

A shot may become its own group only when it independently satisfies the full boundary evidence and safeguard rules. Shot numbering alone is never sufficient.

### 2.4 Candidate D: four scene-label groups

Candidate D is retained as an alternative but cannot yet be accepted.

- Scene labels provide narrative evidence, unlike canvas groups.
- The fantasy-to-reality scene transition is a strong boundary.
- However, different source versions use a prologue plus four acts, four scenes, or replacement ranges that do not align exactly.
- Some scene-sized ranges still contain independent threat, mobilization, reveal, and restoration objectives.

Candidate D can pass only if the approved master script makes the scene divisions exhaustive, non-overlapping, and production-bounded after continuity and rerun analysis.

## 3. Boundary-rule verification cases

| Case | Expected result | Rule exercised |
| --- | --- | --- |
| A canvas group encloses several unrelated script beats | Reject | Canvas layout cannot define script coverage |
| Two adjacent shots share objective, assets, and revision scope | Merge unless another primary reason exists | Avoid over-fragmentation |
| A reality-layer transition changes scale, location meaning, lighting, and sound | Accept when continuity is explicit | Narrative and production transition |
| A group omits an expensive audio operation | Reject | Exhaustive modality coverage cannot be traded for cost |
| A battle variant cuts before impact while another shows impact | Preserve both partitions as unresolved | Competing lineage authority |
| A late-story feedback request changes only the repair symbol | Keep the reveal group intact and rerun the restoration group | Revision isolation and smallest sufficient rerun |
| A proposed group has no terminal artifact or handoff | Reject or merge | Independently reviewable production unit |
| Two partitions both satisfy every invariant with different professional tradeoffs | Retain both for approval | Gold permits acceptable alternatives |

## 4. Approved Ledger Set

The user approved the complete 29-shot, 180-second candidate and the five-group partition on 2026-07-14. Every group requires image, video, and audio evaluation. Artifact-level conflicts remain scored unresolved cases unless an approval record explicitly promotes them. Professional equivalents are accepted when they preserve measurable semantics and continuity; apparent polish, recency, or graph popularity never establishes authority.

## 5. Task protocol and execution modes

Every production task addresses exactly one approved group and one stage. The four contracts are evaluated independently:

| Contract | Required behavior | Minimum evidence |
| --- | --- | --- |
| Interaction | State assumptions, ask only decision-changing questions, and obtain confirmation before mutation | Question log, proposal, confirmation, decision log |
| Workflow | Produce a legal typed graph with ordered dependencies and a bounded mutation | Graph before, ordered patch, graph after, validation result |
| Creative | Preserve approved beats, declared alternatives, and continuity without inventing authority | Beat map, constraint map, alternative rationale |
| Execution | Bound candidates, attempts, cost, and latency; report provenance, failure, and abstention | Attempt log, artifact manifest, measurements, terminal status |

Checkpoint mode starts from an approved prior checkpoint and evaluates one group through its terminal review artifact. Continuation mode inherits the accumulated project state and may receive a full-project score only after all five groups and the cross-group continuity task complete. Both modes use identical group-stage semantics; only inherited state and score eligibility differ.

A task stops when it passes all hard gates, exhausts a declared budget, reaches a terminal provider failure after bounded recovery, or correctly abstains because authority or evidence is insufficient. A failure must name the earliest observable contract, affected artifacts, stale downstream outputs, and smallest sufficient rerun.

## 6. Multimodal measurement rules

Measurements have three layers. Deterministic checks establish artifact validity and declared technical properties. Semantic checks compare only approved facts and declared tolerances. Professional-quality review detects defects that deterministic checks cannot judge while requiring evidence and calibrated abstention.

| Modality | Deterministic | Semantic | Professional quality |
| --- | --- | --- | --- |
| Image | Decodable file, dimensions, aspect ratio, color properties, provenance | Beat, identity, object, environment, composition, and continuity constraints | Anatomy, geometry, texture, lighting, text, aliasing, contamination, and production usability |
| Video | Decodable stream, duration, frame rate, dimensions, black/frozen-frame and timestamp checks | Required action, camera intent, identity, geography, transition, and inherited state | Flicker, warp, judder, temporal coherence, motion quality, edit rhythm, and transition quality |
| Audio | Decodable stream, format, sample rate, channels, duration, loudness, true peak, clipping, and silence | Dialogue/content, voice constraints, motif, ambience, cue timing, and emotional handoff | Intelligibility, noise, distortion, pumping, spectral artifacts, mix balance, and production usability |
| Cross-modal | Common timeline, declared offsets, complete artifact references | Audio-event, visual-event, identity, action, and transition alignment | Synchronization, perceptual coherence, dramatic rhythm, and continuity across cuts and groups |

Missing required media fails the modality hard gate. Invalid or corrupt media receives no quality credit. Provider failure may still earn interaction, bounded-execution, recovery, and reporting credit. Design-only text may guide semantic review only when declared as unresolved evidence; it never counts as rendered media.

Measurements use declared tolerances instead of hidden provider defaults. Duration, dimensions, loudness, synchronization, cost, latency, candidate count, and retry bounds must be recorded by the task or Gold fixture. When the source lacks an exact value, the evaluator scores preservation of semantic intent and professional acceptability rather than inventing a threshold.

## 7. Gold, gates, and reporting

Each Gold fixture separates exact, semantic, and professional layers. Exact Gold covers approved IDs, shot/time coverage, and explicit constraints. Semantic Gold owns narrative meaning and continuity. Professional Gold permits equivalent implementations and calibrated improvements. Raw prompts, private media, URLs, and provider IDs are never Gold.

Hard gates are evaluated before weighted scoring: approved group, legal workflow, complete required modalities, bounded mutation and execution, honest provenance, and correct run labeling. Partial runs report only executed dimensions and can never receive a full-project score. Scores remain separate for interaction, workflow, creative, image, video, audio, cross-modal alignment, execution/recovery, revision, cost, reliability, coverage, and global continuity.

## 8. Verification cases

- A valid image, video, or audio artifact with complete measurements proceeds to semantic and quality grading.
- A corrupt artifact fails deterministic validity and receives no modality-quality credit.
- A missing modality fails its hard gate even when the other modalities are attractive.
- An implementation that differs from the source rendering but preserves approved semantics and tolerances may receive full credit.
- Insufficient authority should produce an explicit unresolved result or abstention, not fabricated exact Gold.
- A checkpoint and continuation run of the same group use the same requirements and Gold; continuation additionally validates inherited state.
- A sampled or interrupted run is labeled partial and cannot receive the full-project aggregate.
