# Oracle visual template assets

These native 240×160 plates are the production floor for the built-in Oracle
templates selected by `art[].template` in `CODEQUEST.toml`.

| Template | Native plate | Live renderer responsibility |
|---|---|---|
| `oracle-chronicle` | `chronicle.png` | Repository title, copyright notice, authors, dates, reveal timing |
| `oracle-awakening` | `awakening-source.png`, `awakening-signal.png`, `awakening-archive.png`, `awakening-convergence.png`, `awakening.png` | Five-scene source ember → archive answer → memory vault → convergence → Oracle story and skip prompt |
| `oracle-title` | `gateway.png` | Cartridge title and start prompt |
| `oracle-menu` | `gateway.png` | Focus, contained action labels, and controls |
| `oracle-atelier` | `atelier.png` | Hero, customization values, truthful generation status, focus |
| `oracle-hero` | `hero-*.png`, `portrait-*.png` | Selected authored colorway with no procedural accessory or weapon overlays |
| `oracle-sanctum` | `sanctum.png`, `drop-*.png` | Authored Datafall collectibles/hazards, hero movement, raw counts, 3/6/9 charge runes, 1/3/5 containment breaches, status, tier grading |
| `oracle-trial` | `trial.png` | Question, choices, focus, Oracle ward runes, x1/x2/x3 flow, 300/900/1800 Insight Runes, raw score, correctness, review lock |
| `oracle-ascension` | `ascension.png` | Hero rise, earned tier, level, batch, hold/continue state |
| `oracle-aftermath` | `aftermath.png` | Defeated hero, final score, earned tier, replay prompt |
| `oracle-progression` | shared plates | Cyan/gold balance, hero identity, and earned tier across the run |

The PNG files are the inspectable source assets. Matching `.rgb` files are the
dependency-free native buffers embedded by the Rust renderer. Hero and portrait
sprites and Datafall drops use matching `.rgba` buffers so they can composite
over scene plates.

Run `scripts/compile-oracle-assets.sh` after changing a PNG. It regenerates the
runtime buffers and refuses incorrect dimensions or byte counts.

## Art direction floor

- Dense, authored code-cathedral environments with readable foreground,
  midground, and background planes.
- Crisp square pixel clusters, carved basalt, aged brass, luminous glass,
  etched circuitry, and restrained violet fabric.
- Near-black/navy structure, cyan system energy, and gold earned/climactic
  energy. The brightest colors are reserved for meaningful states.
- Live panels are empty in source art. Text, focus, correctness, loading, score,
  progression, and failure remain owned by runtime state.
- Live foreground bounds remain inside their assigned panels and do not overlap.
  Foreground palette colors are checked against the immediate dark panel fill;
  readable text may not rely on glow or decorative art for contrast.
- Container coordinates describe the measured usable interior of each plate.
  Renderers center or align against those coordinates, and preview fixtures use
  the repository's real title so short placeholder copy cannot hide overflow.
- Authored hero sprites remain visually intact. Name and path are textual
  identity choices, aura selects the authored colorway, and lower-fidelity
  procedural accessories or weapons are not layered over the source art.
- Datafall uses an authored cyan-and-gold crystal shard for data and an
  asymmetric magenta corruption glyph for bugs. Their silhouettes, values, and
  hues remain distinct without procedural boxes or crossed lines.
- Dynamic HUD instruments reuse the Oracle's diamond-rune shape language:
  health is three fillable ward seals, score is paired with three awakening
  Insight Runes, and Datafall uses charge and breakable containment seals.
  Empty, intermediate, full, gain, and loss states remain legible by fill,
  count, fracture, label, and position rather than color alone.
- Every visible run metric has explicit breakpoints. Native previews and tests
  cover values immediately below, at, and above score, streak, data-charge,
  corruption, ward, batch, and bond thresholds; a changing raw number alone is
  not treated as progression.
- The opening uses five distinct authored compositions. It begins with one
  restrained cyan ember, opens the archive, carries the code-seer into the
  memory vault, adds gold at convergence, and reveals the complete Oracle only
  in `awakening.png` at the crescendo.
- Every reachable scene, including the repository chronicle and aftermath, must
  meet this floor at native resolution. A procedural palette swap is not an
  acceptable substitute for an illustrated plate.

Generated plates were produced with the built-in image-generation workflow.
The four new opening plates used `awakening.png` and the preceding story plate
as strict style and continuity references. Prompts consistently requested:
exact 3:2 game content with no device, no baked text or state, crisp polished
pixel art, the same code-seer and shared material/palette language, a quiet
bottom-right region for the live skip prompt, and maximum white/gold reserved
for the existing final plate.
