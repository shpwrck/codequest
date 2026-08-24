# CODE QUEST visual floor

Use this reference whenever a design or polish pass creates, replaces, or
evaluates visual assets.

## Acceptance floor

Polished CODE QUEST art is an authored game frame, not a palette treatment.
At native 240×160 it must retain:

- one unmistakable focal hierarchy;
- foreground, midground, and background depth;
- dense but controlled material texture;
- crisp pixel clusters and readable silhouettes;
- state-owned lighting with cyan for system energy and gold for earned or
  climactic energy;
- quiet regions intentionally reserved for live text and interaction feedback.
- fidelity continuity between authored assets and runtime overlays; omit a
  procedural adornment when its pixel treatment is visibly coarser or less
  authored than the plate or sprite beneath it;
- strict containment: foreground elements stay inside their assigned panel or
  playfield, and sibling foreground bounds never intersect;
- measured contrast between every foreground element and its immediate
  backdrop. Text must remain readable without relying on glow or an outline to
  rescue a low-contrast fill.
- visible separation between adjacent glyph cells for primary question and
  choice copy. Reduce the manifest's text budget rather than collapsing letter
  spacing to preserve a larger character count.
- usable-content measurements that exclude decorative borders, pedestals,
  dividers, and flourishes. Center live copy within this interior, not the
  asset's larger outer silhouette.
- grounded placements measured from the sprite's visible alpha support point
  to the plate's platform or floor line, rather than from transparent canvas
  edges.
- themed instrumentation: health, score, counters, pips, cursors, and markers
  reuse the frame's authored rune/material language and make deliberate use of
  their available space instead of falling back to generic glyphs;
- distinct empty, intermediate, full, gain, and loss states for every HUD
  instrument, readable through fill, silhouette, count, fracture, position, or
  a concise label rather than hue alone;

The built-in Oracle catalog at `src-tauri/assets/oracle/` is the repository's
minimum production reference. Inspect its PNGs before proposing another Oracle
template. Reject a result that only adds lines, boxes, glow, or the correct
palette to otherwise sparse geometry.

## Asset-backed workflow

1. Render and inspect the current scene at 240×160.
2. Inventory the repository-owned template plates and sprite states.
3. Separate static illustration from live state. Static art owns environment,
   material, framing, and quiet panel interiors. Runtime owns text, focus,
   selection, loading, score, correctness, failure, and progression.
4. Reuse a plate only when its composition supports the new scene. Sharing a
   world is good; forcing unrelated information into the same layout is not.
5. Generate a missing plate with the approved frames as style references. Ask
   for exact 3:2 game content, no device, no baked text or state, and explicit
   empty regions for live UI.
6. Normalize generated state. No answer may be preselected, no character may be
   baked into a playfield, and no fake status may appear in the source image.
7. Downsample with nearest-neighbor filtering and inspect the native result.
   High-resolution concept art is not evidence until the 240×160 frame passes.
8. Compile the PNGs into the runtime format, render every reachable scene, and
   inspect credits, opening stages, title, menus, gameplay, rewards, failure,
   and replay.
9. Run layout checks on the final native composite with worst-case live strings
   and counters. Fail the build when text or focus exceeds its usable interior,
   any pair of live/static foreground siblings overlaps, a plate ornament enters
   a glyph cell, a centered element misses either measured axis, a visible-alpha
   support point misses its placement line, or a foreground/background palette
   pair falls below the project's contrast floor. Measure the usable interior
   and support line from the actual plate; do not substitute the framebuffer
   center, nominal sprite canvas, or an estimated outer border. Preview the real
   manifest title and representative production provenance, not only short
   fixture text.
   Render every tracked quantity immediately below, at, and above each declared
   threshold; verify that its themed instrument, reward/consequence, and
   crossing response change at the exact breakpoint.
10. Compare every runtime-drawn ornament, collectible, and hazard against the
    authored plate at native resolution. Remove accessory, weapon, particle,
    badge, collectible, or hazard overlays that do not meet the same material
    detail, cluster discipline, and silhouette quality; customization metadata
    does not require a visible sprite overlay.

## Progression rule

The opening and the run both earn their brightest frame:

`dormant → first cyan signal → cyan propagation → gold convergence → Oracle climax`

Later gameplay returns to a restrained baseline, then carries earned change in
at least two channels such as light balance, environment energy, crest detail,
hero treatment, or motion density. Do not communicate progression only with a
number.

## Review questions

- Does the native frame look illustrated before live text is added?
- Is the brightest/densest region meaningful?
- Can focus, correctness, and failure change without repainting the source?
- Does the scene remain legible in grayscale and when audio is muted?
- Do all live bounds remain inside their containers without intersecting other
  foreground content?
- Is every centered or aligned element positioned relative to its measured art
  content interior rather than the full framebuffer or decorative shell, on
  both the horizontal and vertical axes?
- Does every character's visible support point meet the platform/floor line?
- Do health, score, counter, pip, cursor, and marker shapes look authored for
  this world and use their available HUD space without sacrificing legibility?
- Does every visible tracked quantity have staged thresholds with a clear
  desirable direction, crossing response, cap, and reset—or should the bare
  counter be removed?
- Does worst-case live copy remain disjoint from every plate divider, border,
  flourish, and sibling foreground element—including between glyphs?
- Does each foreground/background pair meet the contrast check at native size?
- Do runtime-drawn ornaments match the fidelity of the authored art beneath
  them, or should the cleaner authored asset stand on its own?
- Does it hand off a motif or state cleanly to the next scene?
- Does it meet or exceed the Oracle asset catalog rather than merely resemble
  its colors?
