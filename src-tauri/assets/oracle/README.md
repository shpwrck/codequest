# Oracle visual template assets

These native 240×160 plates are the production floor for the built-in Oracle
templates selected by `art[].template` in `CODEQUEST.toml`.

| Template | Native plate | Live renderer responsibility |
|---|---|---|
| `oracle-chronicle` | `chronicle.png` | Repository title, copyright notice, authors, dates, reveal timing |
| `oracle-awakening` | `awakening.png` | Dormant → cyan → gold → Oracle luminance progression and skip prompt |
| `oracle-title` | `gateway.png` | Cartridge title and start prompt |
| `oracle-menu` | `gateway.png` | Focus, menu labels, descriptions, and controls |
| `oracle-atelier` | `atelier.png` | Hero, customization values, truthful generation status, focus |
| `oracle-hero` | `hero-*.png`, `portrait-*.png` | Selected style plus lightweight accessory/weapon states |
| `oracle-sanctum` | `sanctum.png` | Datafall objects, hero movement, counters, status, tier grading |
| `oracle-trial` | `trial.png` | Question, choices, focus, hearts, score, correctness, review lock |
| `oracle-ascension` | `ascension.png` | Hero rise, earned tier, level, batch, hold/continue state |
| `oracle-aftermath` | `aftermath.png` | Defeated hero, final score, earned tier, replay prompt |
| `oracle-progression` | shared plates | Cyan/gold balance, hero identity, and earned tier across the run |

The PNG files are the inspectable source assets. Matching `.rgb` files are the
dependency-free native buffers embedded by the Rust renderer. Hero and portrait
sprites use matching `.rgba` buffers so they can composite over scene plates.

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
- The opening begins without emissive light, introduces cyan, adds gold, and
  reveals the complete Oracle only at the crescendo.
- Every reachable scene, including the repository chronicle and aftermath, must
  meet this floor at native resolution. A procedural palette swap is not an
  acceptable substitute for an illustrated plate.

Generated plates were produced with the built-in image-generation workflow
using the approved Oracle chamber, confrontation, and quiz references as style
inputs. Prompts consistently requested: exact 3:2 game content with no device,
no baked text or state, crisp polished pixel art, the shared material/palette
language above, and scene-specific empty regions for live UI.
