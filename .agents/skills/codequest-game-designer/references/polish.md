# Whole-game polish audit

Treat polish as continuity of intent across the entire playable graph. A game is
not polished because its best frame is impressive; it is polished when every
reachable state communicates what is happening, responds completely, and hands
the player into the next state without exposing a production seam.

## Audit every reachable scene

Create a matrix with one row per scene, including credits, tutorials, loading,
interstitials, pause/back paths, failure, results, replay, and terminal states.
Score each row on the five surfaces below. `Intentional none` is valid for sound
or motion only when the silence/stillness has a purpose and a defined handoff.

### Static presentation

- Establish one readable focal hierarchy at native 240×160 resolution.
- Reuse a coherent palette, shape language, typography, and character silhouette.
- Budget the brightest colors and densest detail for meaningful states.
- Specify focused, disabled, loading, success, failure, and empty variants.
- Preserve information through shape, position, or text rather than color alone.
- Keep every foreground element inside its declared container and keep sibling
  foreground bounds disjoint. Treat clipped text, focus marks outside a panel,
  and decorative overlap with live UI as failed states.
- Distinguish the decorative outer frame from its usable content interior.
  Center every label and action in that measured interior on both axes; a
  mathematically centered label in a larger pedestal or ornament is still
  misaligned when the readable interior is smaller.
- Composite worst-case live copy over the native plate and treat borders,
  dividers, flourishes, and other static foreground pixels as occupied bounds.
  No ornament may pass through a glyph cell, including the spacing cell between
  adjacent letters.
- Anchor placed characters by visible alpha bounds and a measured support line,
  not by the nominal transparent sprite canvas. Feet, wheels, shadows, or other
  support points should meet the intended platform without sinking or floating.
- Check every pair of sibling live/static foreground bounds. Do not rely on a
  curated subset that can miss a corner intersection between a heading, row,
  button, portrait, or status strip.
- Treat HUD instrumentation as part of the authored world. Health, score,
  counters, meters, cursors, pips, and markers should reuse the game's material,
  silhouette, and motif vocabulary while filling their allocated negative space
  deliberately. Generic stars, dots, emoji, stock hearts, and arbitrary bars
  are placeholders unless that symbol is part of the established theme.
- Make every instrument state readable by more than hue: preserve a short
  label where needed and change fill, count, silhouette, fracture, position, or
  another channel across empty, intermediate, full, gain, and loss states.
- Measure foreground contrast against the immediate image or fill beneath it.
  Text must be readable at native size without depending on glow alone.
- Use repository-owned template assets when they fit; generate or implement only
  the missing states. Record actual paths and status instead of implying an
  asset exists because a manifest ID names it.

### Motion and transition design

- Give each sequence a readable progression: dormant state → first signal →
  escalation → climax → handoff. Reserve maximal contrast and motion for the
  climax so intensity is earned.
- Make every motion communicate state, causality, input, or consequence.
- Define timing, anticipation, impact, hold, recovery, skip, and interruption.
- Ensure entering and exiting frames share a motif or deliberately clear it.
- Provide a reduced-motion version with equivalent information and duration;
  avoid full-frame flashes as a substitute for impact.

### Sound design

- Define the sonic identity and constraints: timbre, channel density, motif,
  loudness hierarchy, and the role of silence.
- Pair one player action with one legible cue. Distinguish navigate, confirm,
  cancel, success, damage/failure, reward, and unavailable actions.
- Start and stop ambience, loops, and one-shots at owned state boundaries. Avoid
  stacked confirmation sounds, orphaned loops, and hard cuts without intent.
- Let sound progress with the same arc as the visuals and mechanics; reserve the
  fullest arrangement for earned peaks.
- Specify muted and reduced-audio behavior. Never make sound the only carrier of
  required information.

### Mechanical closure

For every state, enumerate entry causes, active inputs, asynchronous/system
events, feedback, exits, retries, cancellation, and terminal outcomes.

A state is open when any reachable player or system condition has no deliberate
response or recovery—for example a loading state that can fail forever, a held
input that leaks into the next scene, a terminal screen with no exit, or an
animation whose skip path bypasses required initialization.

- Route every event, reject it visibly, or mark it intentionally inactive.
- Give loading, empty, offline/disabled, invalid-data, timeout, and retry states
  honest behavior when they are possible.
- Make success, failure, back, replay, and abandon paths explicit.
- Verify input-edge behavior across transitions and deterministic state reset on
  new runs.
- Test graph reachability and event closure; reachability alone is insufficient.

### Felt progression

- Define a beginning, middle, and end state the player can distinguish without
  reading a level number.
- Change at least two feedback channels across a run: decision complexity,
  encounter rhythm, environment state, character capability, animation, sound,
  reward presentation, or narrative context.
- Carry earned state into later scenes so progress survives transitions.
- Telegraph the next threshold, celebrate crossing it, then establish the new
  baseline. Reset clearly on failure or a new run.
- Inventory every player-visible tracked quantity, including health, score,
  combo/streak, resources, hits, collectibles, timers, ranks, and completion
  counts. For each, name the desirable direction, reachable range, breakpoints,
  staged reward or consequence, crossing feedback, cap, and reset. If changing
  the number has no meaningful threshold response, remove it from the HUD or
  turn it into a real system.
- Test the values immediately below, at, and above every threshold. Include
  decreasing metrics so danger, recovery, and low-value mastery receive staged
  feedback rather than only a terminal zero state.
- Prefer new mastery and changed decisions over stat inflation alone.

## Use a production matrix

Add this table to the game brief for polish work:

| Scene | Static | Motion | Sound | Mechanical closure | Felt progression | Evidence/status |
|---|---|---|---|---|---|---|
| `scene-id` | focal hierarchy and states | progression and handoff | cues, loop, silence | events and recovery | what carries or changes | implemented / configured / proposed |

Then perform two passes:

1. **Closure pass:** resolve open states, ambiguous inputs, missing recovery, and
   broken handoffs before adding spectacle.
2. **Expression pass:** use the visual, motion, and sound constraints to make the
   closed state graph feel authored, escalating, and cohesive.

For the static column, retain native-pixel evidence for usable interiors,
alignment centers, support lines, worst-case glyph bounds, and the complete
sibling-intersection pass. Also retain native states for themed HUD instruments
and exact breakpoint crossings. Outer panel containment or a raw counter by
itself is not evidence.

The whole game passes only when every reachable row has specified intent in all
five surfaces and the implemented claims have observable evidence.
