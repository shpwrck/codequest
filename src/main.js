/* ============================================================
   CODE QUEST ADVANCE — frontend logic (vanilla JS, no bundler)
   Tauri v2 with withGlobalTauri: window.__TAURI__.core.invoke
   and window.__TAURI__.event.listen. Fully offline.
   ============================================================ */
"use strict";
(() => {

  // ---------------------------------------------------------- helpers
  const $ = (id) => document.getElementById(id);
  const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
  const now = () => Date.now();

  const C = {
    ink: '#1A1C2C', navy: '#29366F', royal: '#3B5DC9', sky: '#41A6F6',
    parch: '#F4F4F4', mist: '#94B0C2', gold: '#FFCD75', green: '#38B764',
    red: '#B13E53', plum: '#5D275D', indigo: '#4A4A9C', indigoSh: '#31316B'
  };

  // ---------------------------------------------------------- tauri bridge (with offline demo fallback)
  let invoke, listenTo;
  const T = window.__TAURI__;
  if (T && T.core && T.event) {
    invoke = (cmd, args) => T.core.invoke(cmd, args);
    listenTo = (name, cb) => T.event.listen(name, cb);
  } else {
    // Demo shim: lets the frontend run in a plain browser for development.
    const handlers = {};
    const demo = { timers: [], running: false };
    const emit = (name, payload) => (handlers[name] || []).forEach((cb) => cb({ payload }));
    listenTo = (name, cb) => { (handlers[name] = handlers[name] || []).push(cb); return Promise.resolve(() => {}); };
    const wipe = () => { demo.timers.forEach(clearTimeout); demo.timers = []; demo.running = false; };
    invoke = (cmd, args) => {
      if (cmd === 'list_quests') return Promise.resolve([
        { id: 'q1', name: 'The Echo Cavern', description: 'A gentle first errand.', boss: 'ECHO IMP', command: 'echo hello world' },
        { id: 'q2', name: 'Trial Of Listing', description: 'Survey the dungeon floor.', boss: 'INODE WRAITH', command: 'ls -la' },
        { id: 'q3', name: 'The Failing Rite', description: 'This one bites back.', boss: 'SEGFAULT OGRE', command: 'demo-fail' }
      ]);
      if (cmd === 'start_quest') {
        if (demo.running) return Promise.reject('a quest is already running');
        demo.running = true;
        const bad = /fail/.test(args.command);
        const lines = [
          ['out', '$ ' + args.command], ['out', 'resolving dependencies...'],
          ['out', 'fetch registry/quests 200 OK'], ['out', 'compiling rune_core v1.2.0'],
          ['err', 'warning: unused variable `sword`'], ['out', 'compiling hero_logic v0.9.1'],
          ['out', 'linking artifacts'], bad ? ['err', 'error[E0308]: mismatched types'] : ['out', 'tests: 12 passed'],
          bad ? ['err', 'error: could not compile `quest`'] : ['out', 'finished in 2.41s'], ['out', 'done.']
        ];
        lines.forEach((l, i) => demo.timers.push(setTimeout(() => emit('quest://output', { line: l[1], stream: l[0] }), 350 * (i + 1))));
        demo.timers.push(setTimeout(() => { demo.running = false; emit('quest://done', { code: bad ? 1 : 0, success: !bad }); }, 350 * (lines.length + 1) + 200));
        return Promise.resolve();
      }
      if (cmd === 'abort_quest') {
        if (demo.running) { wipe(); setTimeout(() => emit('quest://done', { code: null, success: false }), 120); }
        return Promise.resolve();
      }
      return Promise.reject('unknown command ' + cmd);
    };
  }

  // ---------------------------------------------------------- save data
  const SAVE_KEY = 'cqa_save_v1';
  const defaults = () => ({ xp: 0, streak: 0, recent: [], bosses: {}, lastFled: null });
  let save = defaults();
  try {
    const raw = localStorage.getItem(SAVE_KEY);
    if (raw) { const s = JSON.parse(raw); if (s && typeof s === 'object') save = Object.assign(defaults(), s); }
  } catch (e) { /* fresh save */ }
  const persist = () => { try { localStorage.setItem(SAVE_KEY, JSON.stringify(save)); } catch (e) { /* full/blocked */ } };

  // level = floor(sqrt(xp/10)) + 1  (fixed contract)
  const levelForXp = (xp) => Math.floor(Math.sqrt(xp / 10)) + 1;
  const xpForLevel = (lv) => 10 * (lv - 1) * (lv - 1);
  const maxHpForLevel = (lv) => 20 + 2 * (lv - 1);

  const TITLES = [
    [1, 'SCRIPT SQUIRE'], [3, 'CODE PAGE'], [5, 'BUG BASHER'], [8, 'LOOP WARDEN'],
    [11, 'MERGE KNIGHT'], [14, 'CI SENTINEL'], [17, 'REBASE PALADIN'],
    [21, 'DAEMON SLAYER'], [26, 'FORCE-PUSH LORD'], [32, 'KERNEL ARCHMAGE']
  ];
  const titleForLevel = (lv) => { let t = TITLES[0][1]; for (const [l, n] of TITLES) if (lv >= l) t = n; return t; };

  const getBoss = (cmd) => save.bosses[cmd] || { wins: 0, losses: 0, warded: false, times: [], cleared: false };
  const touchBoss = (cmd) => (save.bosses[cmd] = save.bosses[cmd] || { wins: 0, losses: 0, warded: false, times: [], cleared: false });
  const median = (arr) => {
    if (!arr || !arr.length) return null;
    const s = [...arr].sort((a, b) => a - b);
    const m = Math.floor(s.length / 2);
    return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
  };
  const bestOf = (arr) => (arr && arr.length ? Math.min(...arr) : null);
  const fmtTime = (s) => (s < 10 ? s.toFixed(1) : String(Math.round(s))) + 'S';

  // ---------------------------------------------------------- hashing + rng
  const fnv1a = (str) => {
    let h = 0x811c9dc5;
    for (let i = 0; i < str.length; i++) { h ^= str.charCodeAt(i); h = Math.imul(h, 0x01000193); }
    return h >>> 0;
  };
  const mulberry32 = (a) => () => {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };

  // ---------------------------------------------------------- sprites (box-shadow pixels)
  // build shadow string from cell list [[x,y,color],...] at given cell size
  const shadowOf = (cells, size) =>
    cells.map(([x, y, c]) => `${x * size}px ${y * size}px 0 0 ${c}`).join(',');

  const applySprite = (el, cells, size) => {
    el.innerHTML = '';
    const px = document.createElement('i');
    px.className = 'px';
    px.style.width = size + 'px';
    px.style.height = size + 'px';
    px.style.boxShadow = shadowOf(cells, size);
    el.appendChild(px);
  };

  // map-string sprites (hero, icons)
  const cellsFromMap = (rows, colors) => {
    const cells = [];
    rows.forEach((row, y) => {
      for (let x = 0; x < row.length; x++) {
        const ch = row[x];
        if (colors[ch]) cells.push([x, y, colors[ch]]);
      }
    });
    return cells;
  };

  // the hero is Clawd, the Claude Code crab; rows 0-2 are headroom for hats
  const CLAWD = '#CE8E6B';
  const HERO_MAP = [
    '................',
    '................',
    '................',
    '..oooooooooooo..',
    '..oooooooooooo..',
    '..oooeooooeooo..',
    '..oooeooooeooo..',
    'oooooeooooeooooo',
    'oooooooooooooooo',
    'oooooooooooooooo',
    '..oooooooooooo..',
    '..oooooooooooo..',
    '..oooooooooooo..',
    '....o.o..o.o....',
    '....o.o..o.o....',
    '....o.o..o.o....'
  ];
  const HERO_FALLEN_MAP = [
    '................', '................', '................', '................',
    '................', '................', '................', '................',
    '................', '................', '................', '................',
    '....o.o..o.o....',
    '..oooooooooooo..',
    'oooooooooooooooo',
    '..oooeooooeooo..'
  ];
  const heroCells = cellsFromMap(HERO_MAP, { o: CLAWD, e: C.ink });
  const heroFallenCells = cellsFromMap(HERO_FALLEN_MAP, { o: CLAWD, e: C.ink });

  // name-keyed wearables and class-keyed weapons: the crab never changes; the
  // name picks what it wears, the class picks what it wields. Cells lead the
  // composed list, and box-shadow paints the FIRST shadow at a coordinate on
  // top, so within each list detail cells come before body cells.
  const WEARABLES = [
    { name: 'MUSTACHE', cells: (a) => [[3, 7, a], [12, 7, a],
      [4, 8, C.ink], [5, 8, C.ink], [6, 8, C.ink], [9, 8, C.ink], [10, 8, C.ink], [11, 8, C.ink]] },
    { name: 'FEDORA', cells: (a) => [[5, 1, a], [6, 1, a], [7, 1, a], [8, 1, a], [9, 1, a], [10, 1, a],
      [5, 0, C.ink], [6, 0, C.ink], [7, 0, C.ink], [8, 0, C.ink], [9, 0, C.ink], [10, 0, C.ink],
      [3, 2, C.ink], [4, 2, C.ink], [5, 2, C.ink], [6, 2, C.ink], [7, 2, C.ink], [8, 2, C.ink],
      [9, 2, C.ink], [10, 2, C.ink], [11, 2, C.ink], [12, 2, C.ink]] },
    { name: 'BOW TIE', cells: (a) => [[7, 9, C.ink], [8, 9, C.ink],
      [5, 9, a], [6, 9, a], [9, 9, a], [10, 9, a]] },
    { name: 'MONOCLE', cells: (a) => [[13, 9, a],
      [9, 4, C.gold], [10, 4, C.gold], [11, 4, C.gold],
      [8, 5, C.gold], [8, 6, C.gold], [8, 7, C.gold],
      [12, 5, C.gold], [12, 6, C.gold], [12, 7, C.gold],
      [9, 8, C.gold], [10, 8, C.gold], [11, 8, C.gold], [12, 8, C.gold]] },
    { name: 'CROWN', cells: (a) => [[6, 2, a], [9, 2, a],
      [4, 1, C.gold], [6, 1, C.gold], [9, 1, C.gold], [11, 1, C.gold],
      [4, 2, C.gold], [5, 2, C.gold], [7, 2, C.gold], [8, 2, C.gold], [10, 2, C.gold], [11, 2, C.gold]] },
    { name: 'SHADES', cells: (a) => [[5, 5, a], [10, 5, a],
      [4, 5, C.ink], [6, 5, C.ink], [7, 5, C.ink], [8, 5, C.ink], [9, 5, C.ink], [11, 5, C.ink],
      [4, 6, C.ink], [5, 6, C.ink], [6, 6, C.ink], [9, 6, C.ink], [10, 6, C.ink], [11, 6, C.ink]] },
    { name: 'HALO', cells: (a) => [[4, 1, a], [11, 1, a],
      [6, 0, C.gold], [7, 0, C.gold], [8, 0, C.gold], [9, 0, C.gold]] }
  ];
  const wearableFor = (name) => WEARABLES[fnv1a(String(name)) % WEARABLES.length];

  // one weapon per class, same order as HERO_CLASSES:
  // CODE KNIGHT, BUG MAGE, PIPE MONK, MERGE PALADIN, LINT RANGER, SHELL DRUID
  const WEAPONS = [
    { name: 'SWORD', cells: (a) => [[14, 7, a],
      [14, 1, C.parch], [14, 2, C.parch], [14, 3, C.parch], [14, 4, C.parch], [14, 5, C.parch],
      [13, 6, C.gold], [14, 6, C.gold], [15, 6, C.gold], [14, 8, C.gold]] },
    { name: 'ORB STAFF', cells: (a) => [[13, 1, a], [14, 1, a], [13, 2, a], [14, 2, a],
      [15, 0, C.gold],
      [14, 3, C.plum], [14, 4, C.plum], [14, 5, C.plum], [14, 6, C.plum], [14, 7, C.plum]] },
    { name: 'PIPE', cells: (a) => [[14, 5, a],
      [13, 1, C.mist], [14, 1, C.mist], [15, 1, C.mist],
      [14, 2, C.mist], [14, 3, C.mist], [14, 4, C.mist], [14, 6, C.mist], [14, 7, C.mist], [14, 8, C.mist]] },
    { name: 'WARHAMMER', cells: (a) => [[13, 2, a],
      [13, 1, C.mist], [14, 1, C.mist], [15, 1, C.mist],
      [14, 2, C.mist], [15, 2, C.mist],
      [14, 3, C.plum], [14, 4, C.plum], [14, 5, C.plum], [14, 6, C.plum], [14, 7, C.plum]] },
    { name: 'BOW', cells: (a) => [[12, 4, a],
      [13, 1, C.plum], [12, 2, C.plum], [12, 3, C.plum], [12, 5, C.plum], [12, 6, C.plum], [13, 7, C.plum],
      [14, 2, C.parch], [14, 3, C.parch], [14, 4, C.parch], [14, 5, C.parch], [14, 6, C.parch]] },
    { name: 'SHELL SHIELD', cells: (a) => [[1, 6, a],
      [1, 4, C.green], [0, 5, C.green], [1, 5, C.green], [2, 5, C.green],
      [0, 6, C.green], [2, 6, C.green], [0, 7, C.green], [1, 7, C.green], [2, 7, C.green],
      [1, 8, C.green]] }
  ];
  const weaponFor = (cls) => WEAPONS[((cls % WEAPONS.length) + WEAPONS.length) % WEAPONS.length];

  const ICONS = {
    crown: cellsFromMap(['X..X..X', 'X.XXX.X', 'XXXXXXX', '.XXXXX.'], { X: C.gold }),
    skull: cellsFromMap(['.XXXXX.', 'XX.X.XX', 'XXXXXXX', '.X.X.X.'], { X: C.parch }),
    shield: cellsFromMap(['XXXXXXX', 'XXXXXXX', '.XXXXX.', '..XXX..', '...X...'], { X: C.sky })
  };
  const iconEl = (name) => {
    const s = document.createElement('span');
    s.className = 'icon';
    applySprite(s, ICONS[name], 1);
    return s;
  };

  // ---------------------------------------------------------- procedural bosses
  const BOSS_A = ['DEPENDENCY', 'SEGFAULT', 'NULLPTR', 'MERGE', 'TIMEOUT', 'LINTER',
    'REGEX', 'KERNEL', 'PACKET', 'CACHE', 'SYNTAX', 'STACK', 'HEAP', 'MUTEX', 'PIPELINE', 'LEGACY'];
  const BOSS_B = ['GOLEM', 'WRAITH', 'HYDRA', 'IMP', 'BASILISK', 'OGRE', 'SPECTER',
    'DRAKE', 'SLIME', 'LICH', 'GARGOYLE', 'DJINN', 'REVENANT', 'BEHOLDER', 'CRAB', 'SPIDER'];
  const BODY_COLS = [C.plum, C.red, C.royal, C.green, C.plum, C.mist, C.royal, C.red];
  const ACCENT_COLS = [C.gold, C.sky, C.parch, C.green, C.red, C.mist, C.gold, C.plum];
  const EYE_COLS = [C.parch, C.sky, C.gold, C.red];

  // Generate a mirrored creature grid: half-columns drawn then mirrored -> symmetry = creature-ness.
  function genGrid(rng, size) {
    const half = size / 2;
    const g = Array.from({ length: size }, () => new Array(size).fill(0));
    const top = Math.floor(size * 0.22), bot = Math.floor(size * 0.92);
    for (let y = top; y < bot; y++) {
      for (let x = 0; x < half; x++) {
        const dx = (half - 1 - x) / half;              // 0 at mirror seam
        const dy = Math.abs(y - size * 0.56) / (size * 0.40);
        const p = 1.12 - (dx * dx + dy * dy) * 1.5 + (rng() - 0.5) * 0.6;
        if (p > 0.45) { g[y][x] = 1; g[y][size - 1 - x] = 1; }
      }
    }
    // erode isolated pixels
    const at0 = (x, y) => (y >= 0 && y < size && x >= 0 && x < size && g[y][x]) ? 1 : 0;
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
      if (!g[y][x]) continue;
      if (!(at0(x, y - 1) + at0(x, y + 1) + at0(x - 1, y) + at0(x + 1, y))) g[y][x] = 0;
    }
    // horns: symmetric spikes above topmost body row
    const hornKind = Math.floor(rng() * 4);
    const hornCols = hornKind === 0 ? [] : hornKind === 1 ? [2] : hornKind === 2 ? [1, 4] : [3];
    for (const hc of hornCols) {
      const x = Math.floor(half * hc / 6);
      let yTop = -1;
      for (let y = 0; y < size; y++) if (g[y][x]) { yTop = y; break; }
      if (yTop > 2) {
        const len = 2 + Math.floor(rng() * 3);
        for (let k = 1; k <= len && yTop - k >= 0; k++) { g[yTop - k][x] = 2; g[yTop - k][size - 1 - x] = 2; }
      }
    }
    // legs: two symmetric stubs under the body
    let yBot = -1;
    const lx = Math.floor(half * 0.45);
    for (let y = size - 1; y >= 0; y--) if (g[y][lx]) { yBot = y; break; }
    if (yBot > 0) for (let k = 1; k <= 2 && yBot + k < size; k++) { g[yBot + k][lx] = 1; g[yBot + k][size - 1 - lx] = 1; }
    return g;
  }

  function colorizeGrid(g, seed) {
    const size = g.length;
    const body = BODY_COLS[seed & 7];
    const accent = ACCENT_COLS[(seed >> 3) & 7];
    const eye = EYE_COLS[(seed >> 6) & 3];
    const cells = [];
    const at = (x, y) => (g[y] && g[y][x] ? g[y][x] : 0);
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
      if (!g[y][x]) continue;
      const edge = !at(x - 1, y) || !at(x + 1, y) || !at(x, y - 1) || !at(x, y + 1);
      let col;
      if (g[y][x] === 2) col = accent;                       // horns
      else if (edge) col = C.ink;                            // outline
      else col = ((x + y * 2 + (seed & 3)) % 5 === 0) ? accent : body;  // belly pattern
      cells.push([x, y, col]);
    }
    // eyes: symmetric pair (or cyclops) forced onto the face band.
    // NOTE: with box-shadow, the FIRST shadow at a coordinate paints on top,
    // so eye cells must lead the list to stay visible over body pixels.
    const half = size / 2;
    const eyeY = Math.floor(size * 0.42);
    const cyclops = ((seed >> 8) & 3) === 0;
    const gap = 2 + ((seed >> 10) & 1);
    const eyes = [];
    const put = (x, y, c) => { eyes.push([x, y, c]); };
    if (cyclops) { put(half - 1, eyeY, eye); put(half, eyeY, eye); put(half - 1, eyeY - 1, C.ink); put(half, eyeY - 1, C.ink); }
    else {
      put(half - gap, eyeY, eye); put(half + gap - 1, eyeY, eye);
      if (((seed >> 12) & 1)) { put(half - gap, eyeY - 1, C.ink); put(half + gap - 1, eyeY - 1, C.ink); } // angry brow
    }
    return eyes.concat(cells);
  }

  const bossCache = {};
  function bossFor(command, overrideName) {
    const key = command + '|' + (overrideName || '');
    if (bossCache[key]) return bossCache[key];
    const h = fnv1a(command);
    const name = overrideName || (BOSS_A[h & 15] + ' ' + BOSS_B[(h >> 4) & 15]);
    const lv = 1 + ((h >> 8) % 28) + Math.min(20, Math.floor(command.length / 6));
    const cells = colorizeGrid(genGrid(mulberry32(h), 24), h);
    const thumbCells = colorizeGrid(genGrid(mulberry32(h ^ 0x9e3779b9), 8), h);
    const b = { name: String(name).toUpperCase(), lv, cells, thumbCells, hash: h };
    bossCache[key] = b;
    return b;
  }

  // ---------------------------------------------------------- dom refs
  const shellEl = $('shell'), scaleEl = $('shell-scale'), screenInner = $('screen-inner');
  const saveL1 = $('save-l1'), saveL2 = $('save-l2'), paletteRow = $('palette-row'), starfield = $('starfield');
  const selectMsg = $('select-msg'), cmdInput = $('cmd-input'), nameEntry = $('name-entry'), questList = $('quest-list');
  const bossWrap = $('boss-wrap'), bossNameEl = $('boss-name'), bossSpriteEl = $('boss-sprite'), heroSpriteEl = $('hero-sprite');
  const turnEl = $('turn-counter'), comboEl = $('combo-counter'), fxLayer = $('fx-layer');
  const playerTag = $('player-tag'), hpCellsEl = $('hp-cells'), xpFillEl = $('xp-fill');
  const logboxEl = $('logbox'), logLinesEl = $('log-lines'), fleeMeterEl = $('flee-meter');
  const hurtOverlay = $('hurt-overlay'), flashOverlay = $('flash-overlay'), laststandEl = $('laststand');
  const victoryEl = $('victory'), stampEl = $('stamp'), tallyEl = $('tally'), tallyRows = $('tally-rows'), tallyMenu = $('tally-menu');
  const defeatEl = $('defeat'), fatalLines = $('fatal-lines'), dmRetry = $('dm-retry'), dmFlee = $('dm-flee');
  const lvlHero = $('lvl-hero'), lvlRows = $('lvl-rows');

  const screens = { off: $('scr-off'), boot: $('scr-boot'), contra: $('scr-contra'), title: $('scr-title'), menu: $('scr-menu'), hero: $('scr-hero'), quiz: $('scr-quiz'), select: $('scr-select'), battle: $('scr-battle'), levelup: $('scr-levelup') };
  // every show() target must have a key here: a missing one silently blanks the
  // screen (show() deactivates all and activates nothing)
  for (const k of ['off', 'boot', 'contra', 'title', 'menu', 'hero', 'quiz', 'select', 'battle', 'levelup'])
    if (!screens[k]) console.error('CQA: screen element missing for', k);

  // ---------------------------------------------------------- state
  let state = 'title';          // title | select | battle | levelup
  let phase = 'run';            // battle sub-phase: run | victory | defeat
  let ready = false;            // listeners registered
  let quests = [];              // from list_quests
  let run = null;               // current battle run
  let runToken = 0;
  const held = {};

  // fx timers cleared on screen transitions
  const fxTimers = new Set();
  const after = (ms, fn) => { const id = setTimeout(() => { fxTimers.delete(id); fn(); }, ms); fxTimers.add(id); return id; };
  const clearFx = () => { for (const id of fxTimers) clearTimeout(id); fxTimers.clear(); };

  function show(name) {
    for (const k in screens) screens[k].classList.toggle('active', k === name);
    state = name;
  }

  // ---------------------------------------------------------- scale-to-fit shell
  function fit() {
    const s = Math.min(window.innerWidth / 584, window.innerHeight / 352);
    const snapped = s >= 1 ? Math.floor(s * 2) / 2 : Math.max(0.35, s); // half-integer steps, integer look
    // Zoom lays out at the final size; transform scaling makes WebKit re-rasterize shell text during repaints.
    scaleEl.style.zoom = snapped;
  }
  window.addEventListener('resize', fit);

  // ---------------------------------------------------------- audio (WebAudio square jingles, no assets)
  let AC = null;
  function audio() {
    try {
      AC = AC || new (window.AudioContext || window.webkitAudioContext)();
      if (AC.state === 'suspended') AC.resume();
      return AC;
    } catch (e) { return null; }
  }
  function playNotes(freqs, step, vol) {
    const ac = audio(); if (!ac) return;
    try {
      freqs.forEach((f, i) => {
        const o = ac.createOscillator(), g = ac.createGain();
        o.type = 'square'; o.frequency.value = f;
        o.connect(g); g.connect(ac.destination);
        const t = ac.currentTime + i * step;
        g.gain.setValueAtTime(vol, t);
        g.gain.setValueAtTime(0.0001, t + step - 0.015);
        o.start(t); o.stop(t + step);
      });
    } catch (e) { /* audio unavailable */ }
  }
  const jingleLevelUp = () => playNotes([523.25, 659.25, 783.99, 1046.5], 0.13, 0.05);
  const thudStamp = () => playNotes([196, 130.8], 0.09, 0.06);

  // ---------------------------------------------------------- title screen
  function buildStarfield() {
    const rng = mulberry32(1234);
    const shadows = [];
    for (let i = 0; i < 46; i++) {
      const x = Math.floor(rng() * 238);
      const y = Math.floor(rng() * 158);
      const col = rng() < 0.6 ? C.mist : (rng() < 0.5 ? C.parch : C.sky);
      shadows.push(`${x}px ${y}px 0 0 ${col}`);
      shadows.push(`${x}px ${y + 160}px 0 0 ${col}`);   // duplicate for seamless loop
    }
    starfield.style.boxShadow = shadows.join(',');
  }
  function buildPaletteRow() {
    paletteRow.innerHTML = '';
    for (const col of [C.indigo, C.indigoSh, C.ink, C.navy, C.royal, C.sky, C.parch, C.mist, C.gold, C.green, C.red, C.plum]) {
      const d = document.createElement('div');
      d.style.background = col;
      paletteRow.appendChild(d);
    }
  }
  function updateSaveBlock() {
    const lv = levelForXp(save.xp);
    saveL1.innerHTML = `LV ${lv} <span class="gold">${titleForLevel(lv)}</span>`;
    saveL2.textContent = `EXP ${save.xp}/${xpForLevel(lv + 1)} STREAK ${save.streak}`;
  }
  // split a repo name across the two logo lines; drop noise words and, when it
  // still cannot fit, keep the leading significant words (semantic summary)
  function titleLines(name) {
    const stop = new Set(['THE', 'A', 'AN', 'OF', 'MY', 'APP', 'REPO', 'PROJECT']);
    const toks = String(name).split(/[-_. ]+/).filter(Boolean);
    let sig = toks.filter((t) => !stop.has(t.toUpperCase()));
    if (!sig.length) sig = toks;
    const lines = [''];
    for (const t of sig) {
      const cur = lines[lines.length - 1];
      const cand = cur ? cur + ' ' + t : t;
      if (cand.length <= 13) lines[lines.length - 1] = cand;
      else if (lines.length < 2) lines.push(t.length <= 13 ? t : t.slice(0, 13));
      else break;
    }
    return lines;
  }

  function enterTitle() {
    clearFx();
    stopQuizTimer();
    updateSaveBlock();
    // the title belongs to the loaded game
    const logoA = document.querySelector('.logo-a'), logoB = document.querySelector('.logo-b'), logoSub = document.querySelector('.logo-sub');
    const saveBlockEl = document.querySelector('.save-block');
    if (cart && cart.mode === 'quiz') {
      const [l1, l2] = titleLines(cart.title);
      logoA.textContent = l1;
      logoB.textContent = l2 || '';
      logoSub.textContent = 'ENDLESS REPO QUIZ';
      saveBlockEl.style.display = 'none';   // quiz title: just title, subtitle, PRESS START
    } else {
      logoA.textContent = 'CODE QUEST';
      logoB.textContent = 'ADVANCE';
      logoSub.textContent = 'EVERY COMMAND IS A BOSS';
      saveBlockEl.style.display = '';
    }
    show('title');
  }

  // ---------------------------------------------------------- power
  let powered = false;
  const powerSwitch = $('power-switch');
  const powerLed = document.querySelector('.power-led');
  function setPower(on) {
    if (powered === on) return;
    powered = on;
    powerSwitch.classList.toggle('on', on);
    powerLed.classList.toggle('off', !on);
    if (on) {
      if (trayOpen) closeTray();
      quests = cart ? cart.quests.slice() : [];   // the loaded cart IS the game
      enterBoot();
    } else powerDown();
  }
  function powerDown() {
    clearFx();
    stopQuizTimer();
    stopContra();
    quiz = null;                                 // the save persists; the live run does not
    runToken++;                                  // drop any late quest events
    Promise.resolve(invoke('abort_quest')).catch(() => {});  // kill a running quest
    show('off');
  }
  powerSwitch.addEventListener('pointerdown', (e) => {
    e.stopPropagation();                         // don't let this click skip the boot
    setPower(!powered);
  });

  // ---------------------------------------------------------- cartridges
  let carts = [];
  let cart = null;
  let trayOpen = false;
  const cartBack = $('cart-back');
  const trayEl = $('cart-tray');
  const trayCarts = $('tray-carts');
  const selectTitle = $('select-title');
  const trayHint = document.querySelector('.tray-hint');
  const trayError = $('tray-error');
  const trayEsc = (s) => String(s).replace(/[<>&"]/g, (ch) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' }[ch]));
  function renderCartBack() {
    if (cart) {
      cartBack.className = 'loaded';
      cartBack.style.setProperty('--cart-color', cart.color || '#6a6fd1');
      cartBack.title = 'CARTRIDGE: ' + cart.title;
    } else {
      cartBack.className = 'empty';
      cartBack.style.removeProperty('--cart-color');
      cartBack.title = 'CARTRIDGE SLOT (EMPTY)';
    }
  }
  function saveCache() {
    localStorage.setItem('cqa-repo-carts', JSON.stringify(carts.map((c) => ({ path: c.path, title: c.title, color: c.color }))));
  }
  function upsertCache(c) {
    const meta = { path: c.path, title: c.title, color: c.color };
    const i = carts.findIndex((x) => x.path === c.path);
    if (i >= 0) carts[i] = meta; else carts.push(meta);
    saveCache();
  }
  function dropCache(path) {
    carts = carts.filter((x) => x.path !== path);
    saveCache();
  }
  function showTrayError(msg) {
    trayError.textContent = String(msg);
    trayError.classList.remove('hidden');
    setTimeout(() => trayError.classList.add('hidden'), 4000);
  }
  function insertCart(c) {
    if (cart) return;                 // must eject first
    qQueue = []; quizSource = ''; quiz = null;
    cart = c;
    localStorage.setItem('cqa-cart-id', c.path);
    upsertCache(c);
    renderCartBack();                 // fresh 'loaded' class replays the insert animation
    playNotes([196, 261.6], 0.09, 0.05);
    closeTray();
    if (cart.mode === 'quiz') {       // the oracle starts writing the moment the cart clicks in
      const save = loadQuizSave();
      fetchBatch(save ? save.lv : 1, 6);
    }
  }
  async function insertByPath(path) {
    if (cart) return;
    try {
      const c = await invoke('load_cartridge', { path });
      insertCart(c);
    } catch (err) {
      showTrayError(err);
      dropCache(path);                // it no longer successfully loads
      buildTray();
    }
  }
  let picking = false;
  async function addFromDisk() {
    if (picking || cart) return;
    picking = true;
    try {
      const c = await invoke('pick_cartridge');
      if (c) insertCart(c);           // null = dialog cancelled
    } catch (err) {
      showTrayError(err);
    } finally { picking = false; }
  }
  function ejectCart() {
    if (!cart) return;
    playNotes([261.6, 196], 0.09, 0.05);
    cartBack.classList.add('ejecting');
    setTimeout(() => {
      cart = null;
      qQueue = []; quizSource = ''; quiz = null;
      localStorage.removeItem('cqa-cart-id');
      renderCartBack();               // resets to 'empty', clearing 'ejecting'
      if (trayOpen) buildTray();      // unlock the cards
    }, 240);
  }
  function buildTray() {
    trayCarts.innerHTML = '';
    const shortPath = (p) => (p.length > 26 ? '…' + p.slice(-25) : p);
    for (const c of carts) {
      const card = document.createElement('div');
      const isCurrent = cart && cart.path === c.path;
      card.className = 'cart-card' + (isCurrent ? ' current' : '') + (cart ? ' locked' : '');
      card.innerHTML = `<div class="cc-strip">CODEQUEST ADVANCE</div><div class="cc-label" style="--cc:${trayEsc(c.color || '#6a6fd1')}"><span class="cc-title">${trayEsc(c.title)}</span><span class="cc-sub">${trayEsc(shortPath(c.path))}</span></div>`;
      if (!cart) card.addEventListener('pointerdown', (e) => { e.stopPropagation(); insertByPath(c.path); });
      trayCarts.appendChild(card);
    }
    const add = document.createElement('div');
    add.className = 'cart-card add' + (cart ? ' locked' : '');
    add.innerHTML = `<div class="cc-strip">&nbsp;</div><div class="cc-label"><span class="cc-title">+ ADD FROM DISK</span><span class="cc-sub">PICK A GIT REPO</span></div>`;
    if (!cart) add.addEventListener('pointerdown', (e) => { e.stopPropagation(); addFromDisk(); });
    trayCarts.appendChild(add);
    const ej = document.createElement('div');
    ej.className = 'cart-card eject' + (cart ? '' : ' locked');
    ej.innerHTML = `<div class="cc-strip">&nbsp;</div><div class="cc-label"><span class="cc-title">EMPTY SLOT</span><span class="cc-sub">${cart ? 'EJECT CARTRIDGE' : 'NO CART LOADED'}</span></div>`;
    if (cart) ej.addEventListener('pointerdown', (e) => { e.stopPropagation(); ejectCart(); });
    trayCarts.appendChild(ej);
    trayError.classList.add('hidden');
    trayHint.textContent = cart ? 'EJECT THE CARTRIDGE BEFORE SWAPPING' : 'CARTRIDGES ARE LOCAL GIT REPOS · ESC TO CLOSE';
  }
  function openTray() {
    if (powered) return;              // no hot-swapping; the console stays still
    buildTray();
    trayEl.classList.remove('hidden');
    trayOpen = true;
  }
  function closeTray() { trayEl.classList.add('hidden'); trayOpen = false; }
  cartBack.addEventListener('pointerdown', (e) => { e.stopPropagation(); if (trayOpen) closeTray(); else openTray(); });
  trayEl.addEventListener('pointerdown', (e) => { if (e.target === trayEl) closeTray(); });

  // ---------------------------------------------------------- endless repo quiz (default game mode)
  const menuTitle = $('menu-title'), menuCont = $('menu-continue'), menuContInfo = $('menu-cont-info'), menuNew = $('menu-new');
  const quizLv = $('quiz-lv'), quizHearts = $('quiz-hearts'), quizScore = $('quiz-score');
  const quizTimerFill = $('quiz-timer-fill'), quizQ = $('quiz-q'), quizChoices = $('quiz-choices');
  const quizOver = $('quiz-over'), qoStats = $('qo-stats'), quizWait = $('quiz-wait');
  let quiz = null;        // active run
  let menuSel = 0;
  let qQueue = [];        // questions served by Rust (ORACLE = claude, ARCHIVE = procedural)
  let fetching = false;
  let quizSource = '';
  let pendingSaved = null;
  const pickOf = (arr) => arr[(Math.random() * arr.length) | 0];
  const quizSaveKey = () => 'cqa-quiz-' + (cart ? cart.path : '');
  const heroKey = () => 'cqa-hero-' + (cart ? cart.path : '');
  const loadQuizSave = () => { try { return JSON.parse(localStorage.getItem(quizSaveKey())); } catch (e) { return null; } };
  const saveQuizRun = () => { if (quiz && !quiz.over) localStorage.setItem(quizSaveKey(), JSON.stringify({ lv: quiz.lv, score: quiz.score, hearts: quiz.hearts, qnum: quiz.qnum, streak: quiz.streak })); };
  const clearQuizSave = () => localStorage.removeItem(quizSaveKey());

  function enterMenu() {
    clearFx();
    stopQuizTimer();
    menuTitle.textContent = cart ? cart.title : 'REPO';
    const save = loadQuizSave();
    menuCont.classList.toggle('disabled', !save);
    menuContInfo.textContent = save ? `LV ${save.lv} · ${save.score}` : '';
    menuSel = save ? 0 : 1;
    renderMenuSel();
    show('menu');
  }
  function renderMenuSel() {
    menuCont.classList.toggle('sel', menuSel === 0);
    menuNew.classList.toggle('sel', menuSel === 1);
  }

  async function fetchBatch(lv, count) {
    if (fetching || !cart) return;
    const forPath = cart.path;
    fetching = true;
    updateOracle();
    try {
      const b = await invoke('quiz_batch', { path: forPath, level: lv, count });
      if (cart && cart.path === forPath) {          // discard stale batches after a swap
        qQueue.push(...b.questions.filter((x) => x && Array.isArray(x.choices) && x.choices.length >= 2));
        quizSource = b.source;
      }
    } catch (err) { /* rust falls back internally; a hard error leaves the queue */ }
    fetching = false;
    updateOracle();
  }

  // ------------------- hero customization: procedural, buys generation time
  const heroPreview = $('hero-preview'), heroOracle = $('hero-oracle');
  const hrName = $('hr-name'), hrClass = $('hr-class'), hrPal = $('hr-pal');
  const heroRows = () => document.querySelectorAll('#scr-hero .hero-row');
  const HERO_CLASSES = ['CODE KNIGHT', 'BUG MAGE', 'PIPE MONK', 'MERGE PALADIN', 'LINT RANGER', 'SHELL DRUID'];
  const HERO_PALS = [
    ['#41A6F6', '#3B5DC9', '#FFCD75'],
    ['#38B764', '#257179', '#F4F4F4'],
    ['#B13E53', '#5D275D', '#FFCD75'],
    ['#FFCD75', '#B13E53', '#29366F'],
    ['#94B0C2', '#566C86', '#41A6F6'],
  ];
  const HERO_PRE = ['GREP', 'SUDO', 'VIM', 'FORK', 'NULL', 'ASYNC', 'TURBO', 'PATCH', 'KERNEL', 'REGEX'];
  const HERO_EPI = ['THE BOLD', 'THE LINTED', 'THE REBASED', 'THE CACHED', 'THE MERGED', 'THE RECURSIVE', 'THE SEGFAULT', 'THE UNSLEEPING'];
  let hero = { name: 'SUDO THE BOLD', cls: 0, pal: 0 };
  let heroRow = 0;
  const rollName = () => pickOf(HERO_PRE) + ' ' + pickOf(HERO_EPI);
  function renderHeroSprite() {
    // the crab is constant; the name picks the wearable, the class the weapon,
    // STYLE tints both accents
    const accent = HERO_PALS[hero.pal][0];
    const cells = wearableFor(hero.name).cells(accent)
      .concat(weaponFor(hero.cls).cells(accent))
      .concat(heroCells);
    return shadowOf(cells, 4);
  }
  function renderHeroSpriteInto(el) { el.style.boxShadow = renderHeroSprite(); }
  function renderHero() {
    hrName.textContent = hero.name;
    hrClass.textContent = HERO_CLASSES[hero.cls];
    hrPal.textContent = 'STYLE ' + (hero.pal + 1);
    heroRows().forEach((el, i) => el.classList.toggle('sel', i === heroRow));
    renderHeroSpriteInto(heroPreview);
  }
  function updateOracle() {
    if (state !== 'hero') return;
    heroOracle.textContent = qQueue.length
      ? `ORACLE READY · ${qQueue.length} QUESTIONS (${quizSource})`
      : (fetching ? 'THE ORACLE IS WRITING QUESTIONS…' : 'ORACLE IDLE');
  }
  function enterHero(saved) {
    clearFx();
    pendingSaved = saved;
    try { const h = JSON.parse(localStorage.getItem(heroKey())); if (h && h.name) hero = h; } catch (e) { /* fresh hero */ }
    if (!qQueue.length) fetchBatch(saved ? saved.lv : 1, 6);
    heroRow = 0;
    renderHero();
    show('hero');
    updateOracle();
  }
  function heroAdjust(dir) {
    if (heroRow === 0) hero.name = rollName();
    else if (heroRow === 1) hero.cls = (hero.cls + dir + HERO_CLASSES.length) % HERO_CLASSES.length;
    else if (heroRow === 2) hero.pal = (hero.pal + dir + HERO_PALS.length) % HERO_PALS.length;
    renderHero();
  }
  function heroBegin() {
    localStorage.setItem(heroKey(), JSON.stringify(hero));
    beginQuizRun(pendingSaved);
  }

  // ------------------- the quiz run
  function stopQuizTimer() {
    if (quiz && quiz.timerId) { clearInterval(quiz.timerId); quiz.timerId = null; }
    if (quiz && quiz.waitIv) { clearInterval(quiz.waitIv); quiz.waitIv = null; }
    if (quiz && quiz.travelIv) { clearInterval(quiz.travelIv); quiz.travelIv = null; }
    quizWait.classList.remove('on');
  }
  function renderQuizHud() {
    quizLv.textContent = 'LV ' + quiz.lv;
    quizHearts.textContent = '♥'.repeat(quiz.hearts) + '♡'.repeat(Math.max(0, 3 - quiz.hearts));
    quizScore.textContent = String(quiz.score);
  }
  const TRAVEL_FOES = ['GREP GOLEM', 'SEGFAULT WRAITH', 'BORROW CHECKER', 'FLAKY HYDRA', 'STYLE BASILISK', 'OOM REAPER'];
  function travelLine() {
    return pickOf([
      `${hero.name} VENTURES DEEPER INTO ${cart ? cart.title : 'THE REPO'}…`,
      `THE ${HERO_CLASSES[hero.cls]} STUDIES ANCIENT SCROLLS…`,
      `A ${pickOf(TRAVEL_FOES)} RUMBLES IN THE DISTANCE…`,
      'THE ORACLE INSCRIBES NEW TRIALS…',
      `CAMPFIRE: ${hero.name} SHARPENS THE MIND…`,
      'STRANGE RUNES GLOW ALONG THE PATH…',
    ]);
  }
  function nextQuestion() {
    stopQuizTimer();
    if (!qQueue.length) {
      // no waiting screens: the journey continues until the next challenger
      quizWait.classList.add('on');
      renderHeroSpriteInto($('travel-hero'));
      const lineEl = $('travel-line');
      lineEl.textContent = travelLine();
      quiz.travelIv = setInterval(() => { lineEl.textContent = travelLine(); }, 1700);
      if (!fetching) fetchBatch(quiz.lv, 6);
      quiz.waitIv = setInterval(() => {
        if (qQueue.length) {
          clearInterval(quiz.waitIv); quiz.waitIv = null;
          lineEl.textContent = 'A CHALLENGER APPEARS!';
          playNotes([523.25, 659.25], 0.09, 0.04);
          after(750, () => { stopQuizTimer(); if (state === 'quiz' && !quiz.over) nextQuestion(); });
        } else if (!fetching) fetchBatch(quiz.lv, 6);
      }, 300);
      return;
    }
    quiz.cur = qQueue.shift();
    quiz.sel = 0;
    quiz.locked = false;
    quizQ.textContent = quiz.cur.q;
    quizChoices.innerHTML = '';
    quiz.cur.choices.forEach((c, i) => {
      const el = document.createElement('div');
      el.className = 'quiz-choice' + (i === 0 ? ' sel' : '');
      el.textContent = c;
      el.addEventListener('pointerdown', () => { if (!quiz.locked) { quiz.sel = i; renderQuizSel(); confirmAnswer(); } });
      quizChoices.appendChild(el);
    });
    renderQuizHud();
    const secs = Math.max(6, 16 - quiz.lv);
    quiz.deadline = Date.now() + secs * 1000;
    quiz.total = secs * 1000;
    quizTimerFill.style.width = '100%';
    quiz.timerId = setInterval(() => {
      const left = quiz.deadline - Date.now();
      quizTimerFill.style.width = Math.max(0, (left / quiz.total) * 100) + '%';
      if (left <= 0) { if (quiz.timerId) clearInterval(quiz.timerId); quiz.timerId = null; resolveAnswer(-1); }
    }, 100);
    if (qQueue.length < 5 && !fetching) fetchBatch(quiz.lv + 1, 6);   // prefetch the next, harder batch
  }
  function renderQuizSel() {
    [...quizChoices.children].forEach((el, i) => el.classList.toggle('sel', i === quiz.sel));
  }
  function confirmAnswer() { if (!quiz.locked) resolveAnswer(quiz.sel); }
  function resolveAnswer(idx) {
    if (quiz.locked) return;
    quiz.locked = true;
    stopQuizTimer();
    const els = [...quizChoices.children];
    const rightIdx = quiz.cur.answer;
    const correct = idx === rightIdx;
    if (els[rightIdx]) els[rightIdx].classList.add('right');
    if (!correct && els[idx]) els[idx].classList.add('wrong');
    if (correct) {
      const bonus = Math.max(0, Math.round((quiz.deadline - Date.now()) / 1000));
      quiz.score += 10 * quiz.lv + bonus;
      quiz.streak++;
      playNotes([659.25, 880], 0.09, 0.04);
      if (quiz.streak % 4 === 0) quiz.lv++;
    } else {
      quiz.hearts--;
      quiz.streak = 0;
      playNotes([220, 174.6], 0.12, 0.05);
      screenInner.classList.remove('shake'); void screenInner.offsetWidth; screenInner.classList.add('shake');
    }
    quiz.qnum++;
    renderQuizHud();
    if (quiz.hearts <= 0) { after(900, quizGameOver); return; }
    saveQuizRun();
    after(900, () => { if (state === 'quiz' && !quiz.over) nextQuestion(); });
  }
  function quizGameOver() {
    quiz.over = true;
    // checkpoint: death keeps the level you reached — continue restarts there
    // with fresh hearts, but the score is forfeit (arcade rules)
    localStorage.setItem(quizSaveKey(), JSON.stringify({ lv: quiz.lv, score: 0, hearts: 3, qnum: 0, streak: 0 }));
    qoStats.innerHTML = `${hero.name} HAS FALLEN<br>FINAL SCORE ${quiz.score}<br>REACHED LV ${quiz.lv} · ${quiz.qnum} QUESTIONS<br>CONTINUE FROM LV ${quiz.lv}`;
    quizOver.classList.add('on');
  }
  function beginQuizRun(saved) {
    quiz = saved
      ? { lv: saved.lv, score: saved.score, hearts: saved.hearts, qnum: saved.qnum, streak: saved.streak || 0, over: false }
      : { lv: 1, score: 0, hearts: 3, qnum: 0, streak: 0, over: false };
    quizOver.classList.remove('on');
    show('quiz');
    nextQuestion();
  }

  // ---------------------------------------------------------- boot sequence
  const jingleBoot = () => playNotes([523.25, 783.99, 1046.5, 1568], 0.11, 0.05);
  let bootSkip = null;                      // last boot's skip handler; re-boots must not stack listeners
  function enterBoot() {
    show('boot');
    after(650, jingleBoot);                 // chime as the logo lands
    // without a cartridge the console halts on the boot screen, like real hardware
    if (bootSkip) {
      window.removeEventListener('keydown', bootSkip);
      window.removeEventListener('pointerdown', bootSkip);
    }
    const done = () => { if (state === 'boot' && cart) enterTitle(); };
    bootSkip = done;
    after(2600, done);                      // auto-advance
    window.addEventListener('keydown', done, { once: true });   // any input skips
    window.addEventListener('pointerdown', done, { once: true });
    kbuf = [];                              // a fresh boot re-arms the secret
  }

  // ---------------------------------------------------------- konami code -> STICK CONTRA (secret)
  // Armed only on the halted boot screen: powered on, no cartridge loaded.
  const KONAMI = ['up', 'up', 'down', 'down', 'left', 'right', 'left', 'right', 'b', 'a', 'select', 'start'];
  let kbuf = [];
  const jingleSecret = () => playNotes([392, 523.25, 659.25, 783.99, 659.25, 783.99, 1046.5, 1318.51], 0.07, 0.06);
  function konamiFeed(btn) {
    kbuf.push(btn);
    if (kbuf.length > KONAMI.length) kbuf.shift();
    if (kbuf.length === KONAMI.length && KONAMI.every((b, i) => kbuf[i] === b)) {
      kbuf = [];
      jingleSecret();
      enterContra();
    }
  }

  const contraCanvas = $('contra-canvas');
  // 234x154: the #screen content box after its 3px border (border-box), so the
  // canvas maps 1:1 to CSS pixels instead of taking a 0.975 nearest-neighbor squeeze
  const CW = 234, CH = 154, CG_GROUND = 141;
  let cg = null, cgRaf = 0;

  function enterContra() {
    const ctx = contraCanvas.getContext('2d');
    ctx.imageSmoothingEnabled = false;
    const srng = mulberry32(0xC0DE);
    const stars = [];
    for (let i = 0; i < 26; i++) stars.push([Math.floor(srng() * CW), Math.floor(srng() * 96)]);
    let hi = 0;
    try { hi = Number(localStorage.getItem('cqa-contra-hi')) || 0; } catch (e) { /* blocked */ }
    cg = {
      ctx, stars, t: 0, lastTs: 0, over: false, paused: false,
      score: 0, hi, rest: 30,   // 30 lives, as tradition demands
      player: { x: 40, y: CG_GROUND, vy: 0, face: 1, run: 0, moving: false, dead: 0, inv: 0, fireCd: 0 },
      shots: [], eshots: [], foes: [], bursts: [], spawnNext: 150
    };
    show('contra');
    cancelAnimationFrame(cgRaf);
    cgRaf = requestAnimationFrame(contraFrame);
  }
  function stopContra() {
    cancelAnimationFrame(cgRaf); cgRaf = 0;
    if (cg && cg.score > cg.hi) { try { localStorage.setItem('cqa-contra-hi', String(cg.score)); } catch (e) { /* full/blocked */ } }
    cg = null;
  }
  function exitContra() { stopContra(); enterBoot(); }   // leaving the secret reboots the console

  function contraFrame(ts) {
    if (state !== 'contra' || !cg) return;
    const dt = cg.lastTs ? clamp((ts - cg.lastTs) / 16.667, 0.01, 3) : 1;   // no floor inflation on >240Hz panels
    cg.lastTs = ts;
    if (!cg.paused && !cg.over) contraStep(dt);
    contraDraw();
    cgRaf = requestAnimationFrame(contraFrame);
  }

  function cgBurst(x, y) {
    for (let i = 0; i < 8; i++) {
      const a = (i / 8) * Math.PI * 2;
      cg.bursts.push({ x, y, vx: Math.cos(a) * (0.7 + Math.random()), vy: Math.sin(a) * (0.7 + Math.random()) - 0.6, life: 16 });
    }
  }
  function contraFire() {
    const p = cg.player;
    if (cg.over || cg.paused || p.dead || p.fireCd > 0) return;
    p.fireCd = 9;
    if (held.up) cg.shots.push({ x: p.x, y: p.y - 17, vx: 0, vy: -3.6 });
    else cg.shots.push({ x: p.x + p.face * 7, y: p.y - 8, vx: p.face * 3.6, vy: 0 });
    if (cg.shots.length > 30) cg.shots.shift();
    playNotes([1568], 0.03, 0.015);
  }
  function cgJump() {
    const p = cg.player;
    if (cg.over || cg.paused || p.dead || p.y < CG_GROUND) return;
    p.vy = -4.4;
    playNotes([523.25], 0.04, 0.015);
  }
  function cgKillPlayer() {
    const p = cg.player;
    if (p.dead) return;
    cg.rest -= 1;
    p.dead = 55;
    cgBurst(p.x, p.y - 8);
    playNotes([220, 164.81, 110], 0.1, 0.05);
  }
  function cgSpawnFoe() {
    if (cg.foes.length >= 10) return;
    const fromLeft = Math.random() < 0.3;
    const shooter = Math.random() < 0.35;
    cg.foes.push({
      x: fromLeft ? -6 : CW + 6, y: CG_GROUND, vy: 0, dir: fromLeft ? 1 : -1,
      type: shooter ? 'shooter' : 'runner', run: Math.random() * 6, cd: 60, still: false,
      targetX: fromLeft ? 30 + Math.random() * 55 : 155 + Math.random() * 55
    });
  }

  function contraStep(dt) {
    cg.t += dt;
    const p = cg.player;

    // player
    if (p.dead) {
      p.dead -= dt;
      if (p.dead <= 0) {
        p.dead = 0;
        if (cg.rest <= 0) {
          cg.over = true;
          cg.overAt = now();
          if (cg.score > cg.hi) { cg.hi = cg.score; try { localStorage.setItem('cqa-contra-hi', String(cg.hi)); } catch (e) { /* full/blocked */ } }
          playNotes([392, 311.13, 233.08, 196], 0.15, 0.05);
          return;
        }
        p.x = 40; p.y = CG_GROUND; p.vy = 0; p.face = 1; p.inv = 100;
        cg.eshots = [];                                              // nothing mid-air at respawn
        cg.foes = cg.foes.filter((f) => Math.abs(f.x - p.x) > 60);   // clear any spawn camp
      }
    } else {
      p.moving = false;
      if (held.left) { p.x -= 1.4 * dt; p.face = -1; p.moving = true; }
      if (held.right) { p.x += 1.4 * dt; p.face = 1; p.moving = true; }
      p.x = clamp(p.x, 5, CW - 5);
      if (p.moving) p.run += 0.32 * dt;
      p.vy += 0.28 * dt;
      p.y += p.vy * dt;
      if (p.y >= CG_GROUND) { p.y = CG_GROUND; p.vy = 0; }
      if (held.a) contraFire();
      p.inv = Math.max(0, p.inv - dt);
    }
    p.fireCd = Math.max(0, p.fireCd - dt);

    // spawns (the READY beat holds them off at first)
    if (cg.t >= cg.spawnNext) {
      cgSpawnFoe();
      cg.spawnNext = cg.t + Math.max(28, 85 - cg.t / 90) * (0.75 + Math.random() * 0.5);
    }

    // foes
    for (let i = cg.foes.length - 1; i >= 0; i--) {
      const f = cg.foes[i];
      if (f.type === 'runner') {
        if (!p.dead) f.dir = (p.x < f.x ? -1 : 1);
        f.x += 0.9 * f.dir * dt;
        f.run += 0.3 * dt;
        if (f.y >= CG_GROUND && Math.random() < 0.004 * dt) f.vy = -3.4;   // the odd hop
      } else if (Math.abs(f.x - f.targetX) > 2) {
        f.still = false;
        f.dir = f.targetX > f.x ? 1 : -1;
        f.x += 0.8 * f.dir * dt;
        f.run += 0.28 * dt;
      } else {
        f.still = true;
        f.dir = (p.x < f.x ? -1 : 1);
        if (!p.dead) {                       // cooldowns freeze while the player is down: no respawn volley
          f.cd -= dt;
          if (f.cd <= 0) {
            f.cd = 75 + Math.random() * 55;
            cg.eshots.push({ x: f.x + f.dir * 6, y: f.y - 8, vx: f.dir * 1.5, fresh: true });
            playNotes([392], 0.04, 0.02);
          }
        }
      }
      f.vy += 0.28 * dt;
      f.y += f.vy * dt;
      if (f.y >= CG_GROUND) { f.y = CG_GROUND; f.vy = 0; }
      if (f.x < -12 || f.x > CW + 12) { cg.foes.splice(i, 1); continue; }
      if (!p.dead && p.inv <= 0 && Math.abs(f.x - p.x) < 5 && Math.abs(f.y - p.y) < 12) cgKillPlayer();
    }

    // player shots
    for (let i = cg.shots.length - 1; i >= 0; i--) {
      const s = cg.shots[i];
      const x0 = s.x;
      s.x += s.vx * dt;
      s.y += s.vy * dt;
      if (s.x < -4 || s.x > CW + 4 || s.y < -4) { cg.shots.splice(i, 1); continue; }
      for (let j = cg.foes.length - 1; j >= 0; j--) {
        const f = cg.foes[j];
        // swept in x so a slow frame can't tunnel a bullet through a foe
        if (f.x > Math.min(x0, s.x) - 4 && f.x < Math.max(x0, s.x) + 4 && s.y > f.y - 15 && s.y < f.y + 1) {
          cg.score += f.type === 'shooter' ? 300 : 100;
          cgBurst(f.x, f.y - 8);
          cg.foes.splice(j, 1);
          cg.shots.splice(i, 1);
          playNotes([587.33, 880], 0.04, 0.03);
          break;
        }
      }
    }

    // enemy shots
    for (let i = cg.eshots.length - 1; i >= 0; i--) {
      const s = cg.eshots[i];
      if (s.fresh) { s.fresh = false; continue; }   // draw at the muzzle once before it can hit
      s.x += s.vx * dt;
      if (s.x < -4 || s.x > CW + 4) { cg.eshots.splice(i, 1); continue; }
      if (!p.dead && p.inv <= 0 && Math.abs(s.x - p.x) < 4 && s.y > p.y - 15 && s.y < p.y + 1) {
        cg.eshots.splice(i, 1);
        cgKillPlayer();
      }
    }

    // debris
    for (let i = cg.bursts.length - 1; i >= 0; i--) {
      const b = cg.bursts[i];
      b.life -= dt; b.x += b.vx * dt; b.y += b.vy * dt; b.vy += 0.08 * dt;
      if (b.life <= 0) cg.bursts.splice(i, 1);
    }
  }

  // one stick figure, feet at (x,y); pure white lines on black
  function drawStick(ctx, f) {
    const x = Math.round(f.x) + 0.5, y = Math.round(f.y) + 0.5;
    ctx.beginPath();
    if (f.fallen) {
      ctx.arc(x - f.face * 6, y - 2, 2, 0, Math.PI * 2);
      ctx.moveTo(x - f.face * 3, y - 2);
      ctx.lineTo(x + f.face * 6, y - 2);
      ctx.stroke();
      return;
    }
    const swing = f.run != null ? Math.sin(f.run) * 3 : 0;
    ctx.arc(x, y - 12, 2, 0, Math.PI * 2);              // head
    ctx.moveTo(x, y - 10); ctx.lineTo(x, y - 5);        // torso
    if (f.air) {                                        // tucked jump legs
      ctx.moveTo(x, y - 5); ctx.lineTo(x - 3, y - 2);
      ctx.moveTo(x, y - 5); ctx.lineTo(x + 3, y - 1);
    } else {
      ctx.moveTo(x, y - 5); ctx.lineTo(x - 2 + swing, y);
      ctx.moveTo(x, y - 5); ctx.lineTo(x + 2 - swing, y);
    }
    if (f.gunUp) {                                      // rifle skyward
      ctx.moveTo(x, y - 8); ctx.lineTo(x, y - 16);
      ctx.moveTo(x, y - 8); ctx.lineTo(x - f.face * 3, y - 6);
    } else {                                            // rifle level
      ctx.moveTo(x, y - 8); ctx.lineTo(x + f.face * 6, y - 8);
      ctx.moveTo(x, y - 8); ctx.lineTo(x - f.face * 2, y - 5);
    }
    ctx.stroke();
  }

  function contraDraw() {
    const ctx = cg.ctx;
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, CW, CH);
    ctx.fillStyle = '#fff';
    ctx.strokeStyle = '#fff';
    ctx.lineWidth = 1;
    for (const [sx, sy] of cg.stars) ctx.fillRect(sx, sy, 1, 1);
    ctx.fillRect(0, CG_GROUND + 1, CW, 1);
    for (let x = 4; x < CW; x += 24) ctx.fillRect(x, CG_GROUND + 3, 1, 2);

    for (const b of cg.bursts) ctx.fillRect(Math.round(b.x), Math.round(b.y), 1, 1);
    for (const f of cg.foes) drawStick(ctx, { x: f.x, y: f.y, face: f.dir, run: f.still ? null : f.run, air: f.y < CG_GROUND - 0.5 });
    for (const s of cg.shots) ctx.fillRect(Math.round(s.x) - 1, Math.round(s.y), s.vy ? 1 : 3, s.vy ? 3 : 1);
    for (const s of cg.eshots) ctx.fillRect(Math.round(s.x) - 1, Math.round(s.y) - 1, 2, 2);

    const p = cg.player;
    if (p.dead || cg.over) drawStick(ctx, { x: p.x, y: CG_GROUND, face: p.face, fallen: true });
    else if (!(p.inv > 0 && ((cg.t / 3) | 0) % 2)) {
      drawStick(ctx, { x: p.x, y: p.y, face: p.face, run: p.moving ? p.run : null, air: p.y < CG_GROUND - 0.5, gunUp: !!held.up });
    }

    ctx.font = '8px "Press Start 2P", monospace';
    ctx.textAlign = 'left';
    ctx.fillText('REST ' + Math.max(0, cg.rest), 5, 12);
    ctx.textAlign = 'center';
    ctx.fillText('HI ' + Math.max(cg.hi, cg.score), CW / 2, 12);
    ctx.textAlign = 'right';
    ctx.fillText(String(cg.score), CW - 5, 12);

    ctx.textAlign = 'center';
    if (cg.over) {
      ctx.fillText('GAME OVER', CW / 2, 70);
      ctx.fillText('SCORE ' + cg.score, CW / 2, 86);
      if (((now() / 500) | 0) % 2) ctx.fillText('START:RETRY SELECT:EXIT', CW / 2, 106);
    } else if (cg.paused) {
      if (((now() / 400) | 0) % 2) ctx.fillText('PAUSE', CW / 2, 78);
      ctx.fillText('START:RESUME SELECT:EXIT', CW / 2, 94);
    } else if (cg.t < 110) {
      ctx.fillText('PLAYER 1', CW / 2, 62);
      if (((cg.t / 15) | 0) % 2) ctx.fillText('READY', CW / 2, 78);
      ctx.fillText('A:FIRE B:JUMP SELECT:EXIT', CW / 2, 152);
    }
    ctx.textAlign = 'left';
  }

  // ---------------------------------------------------------- quest select
  let menu = [];      // [{type:'custom'}|{type:'quest',quest}|{type:'recent',cmd}]
  let sel = 0;
  let typing = null;  // typewriter interval

  function enterSelect() {
    clearFx();
    show('select');
    selectTitle.textContent = cart ? cart.title : 'CHOOSE THY QUEST';
    buildMenu();
    sel = menu.length > 1 ? 1 : 0;
    renderMenu();
    const msgs = [];
    if (save.lastFled) {
      msgs.push(`YOU FLED THE BATTLE. KEPT ${save.lastFled.kept} EXP.`);
      save.lastFled = null;
      persist();
    }
    msgs.push('A CHALLENGER AWAITS. NAME THY QUEST:');
    typeMsgSeq(msgs);
  }

  function typeMsgSeq(msgs) {
    if (typing) { clearInterval(typing); typing = null; }
    selectMsg.textContent = '';
    let mi = 0, ci = 0;
    typing = setInterval(() => {
      if (state !== 'select') { clearInterval(typing); typing = null; return; }
      const m = msgs[mi];
      if (ci <= m.length) { selectMsg.textContent = m.slice(0, Math.max(0, ci)); ci += 1; }
      else if (mi < msgs.length - 1) {
        ci = -50;  // pause ~1s between messages
        mi += 1;
      } else { clearInterval(typing); typing = null; }
      if (ci < 0) ci += 1;
    }, 18);
  }
  function setMsg(text) {
    if (typing) return;   // don't stomp the ceremony typewriter
    selectMsg.textContent = text;
  }

  function buildMenu() {
    menu = [{ type: 'custom' }];
    for (const q of quests) menu.push({ type: 'quest', quest: q });
    const seen = new Set(quests.map((q) => q.command));
    for (const cmd of save.recent) if (!seen.has(cmd)) menu.push({ type: 'recent', cmd });
  }

  function rowLabel(item) {
    if (item.type === 'custom') return 'CUSTOM QUEST';
    if (item.type === 'quest') return String(item.quest.name || item.quest.command).toUpperCase();
    return item.cmd;
  }
  function rowCommand(item) {
    if (item.type === 'quest') return String(item.quest.command || '');
    if (item.type === 'recent') return item.cmd;
    return null;
  }

  function renderMenu() {
    questList.innerHTML = '';
    menu.forEach((item, i) => {
      if (item.type === 'custom') return;   // custom row = the input field itself
      const row = document.createElement('div');
      row.className = 'qrow' + (i === sel ? ' sel' : '');
      const hand = document.createElement('span');
      hand.className = 'hand';
      hand.textContent = '>';
      if (i !== sel) hand.style.display = 'none';
      row.appendChild(hand);

      const cmd = rowCommand(item);
      const thumb = document.createElement('span');
      thumb.className = 'thumb';
      const boss = bossFor(cmd, item.type === 'quest' ? item.quest.boss : null);
      applySprite(thumb, boss.thumbCells, 2);
      row.appendChild(thumb);

      const label = document.createElement('span');
      label.className = 'qlabel';
      label.textContent = rowLabel(item);
      row.appendChild(label);

      const badges = document.createElement('span');
      badges.className = 'badges';
      const rec = getBoss(cmd);
      if (rec.cleared) badges.appendChild(iconEl('crown'));
      if (rec.losses > 0) {
        badges.appendChild(iconEl('skull'));
        const c = document.createElement('span');
        c.className = 'cnt';
        c.textContent = 'x' + rec.losses;
        badges.appendChild(c);
      }
      const best = bestOf(rec.times);
      if (best != null) {
        const b = document.createElement('span');
        b.className = 'best';
        b.textContent = 'B:' + fmtTime(best);
        badges.appendChild(b);
      }
      if (rec.warded) badges.appendChild(iconEl('shield'));
      row.appendChild(badges);

      row.addEventListener('click', () => {
        if (sel === i) activateRow();
        else { sel = i; renderMenu(); }
      });
      questList.appendChild(row);
    });

    // input focus state for the CUSTOM row
    nameEntry.classList.toggle('focus', sel === 0);
    if (sel === 0) { cmdInput.focus(); }
    else if (document.activeElement === cmdInput) cmdInput.blur();

    // scroll window (rows exclude the custom entry -> index-1)
    const rows = menu.length - 1;
    const off = clamp(sel - 1 - 2, 0, Math.max(0, rows - 5));
    questList.style.transform = `translateY(${-off * 17}px)`;

    // context line in the top dialog
    const item = menu[sel];
    if (item.type === 'custom') setMsg('TYPE A COMMAND, HERO. ENTER TO FIGHT.');
    else if (item.type === 'quest') {
      const q = item.quest;
      setMsg(`${String(q.boss || '').toUpperCase()}: ${q.description || q.command}`.slice(0, 62));
    } else {
      const rec = getBoss(item.cmd);
      const m = median(rec.times);
      setMsg((bossFor(item.cmd).name + ' LV.' + bossFor(item.cmd).lv + (m != null ? ' MED ' + fmtTime(m) : '')).slice(0, 62));
    }
  }

  function moveSel(dir) {
    if (state !== 'select' || !menu.length) return;
    sel = clamp(sel + dir, 0, menu.length - 1);
    renderMenu();
  }

  function toggleWard() {
    const item = menu[sel];
    const cmd = rowCommand(item);
    if (!cmd) return;
    const rec = touchBoss(cmd);
    rec.warded = !rec.warded;
    persist();
    renderMenu();
  }

  function activateRow() {
    const item = menu[sel];
    if (!item) return;
    if (item.type === 'custom') return launchCustom();
    const cmd = rowCommand(item);
    launch(cmd, item.type === 'quest' ? item.quest.boss : null);
  }
  function launchCustom() {
    const cmd = cmdInput.value.trim();
    if (!cmd) {
      nameEntry.style.borderColor = C.red;
      after(300, () => { nameEntry.style.borderColor = ''; });
      return;
    }
    launch(cmd, null);
  }
  function launch(cmd, bossName) {
    if (!ready || !cmd) return;
    save.recent = [cmd, ...save.recent.filter((c) => c !== cmd)].slice(0, 5);
    persist();
    startBattle(cmd, bossName);
  }

  // ---------------------------------------------------------- battle: log queue + rune decode
  const log = { queue: [], cur: null, overflowed: false };
  const clean = (s) => s
    .replace(/\x1b\[[0-9;?]*[ -\/]*[@-~]/g, '')
    .replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '')
    .replace(/\t/g, '  ');
  const fitLine = (s, n) => (s.length > n ? s.slice(0, n - 2) + '..' : s);

  function enqueueLog(text, kind) {
    if (log.queue.length >= 500) {
      if (!log.overflowed) { log.overflowed = true; log.queue.push({ text: '...THE FOE RAGES ON (LINES SKIPPED)...', kind: 'sys' }); }
      return;
    }
    if (log.queue.length < 200) log.overflowed = false;
    log.queue.push({ text: fitLine(clean(text), 28), kind });
  }
  const sysLine = (t) => enqueueLog(t, 'sys');

  function makeLine(e) {
    const d = document.createElement('div');
    d.className = 'logline ' + e.kind;
    const a = document.createElement('span');
    const b = document.createElement('span');
    b.className = 'rune';
    d.appendChild(a); d.appendChild(b);
    logLinesEl.appendChild(d);
    while (logLinesEl.children.length > 30) logLinesEl.removeChild(logLinesEl.firstChild);
    return d;
  }
  function renderCur(c) {
    c.el.children[0].textContent = c.text.slice(0, c.pos);
    c.el.children[1].textContent = c.text.slice(c.pos);
  }

  function logTick() {
    if (state !== 'battle' || !run) return;
    // frenzy expiry
    if (run.frenzyUntil && now() > run.frenzyUntil) {
      run.frenzyUntil = 0;
      logboxEl.classList.remove('frenzy');
      comboEl.classList.remove('gold');
      run.combo = 0;
      updateCombo(false);
    }
    if (run.paused && phase === 'run') return;
    const instant = held.a || log.queue.length > 25 || phase !== 'run';
    let budget = instant ? 60 : 1;
    while (budget-- > 0) {
      if (!log.cur) {
        const e = log.queue.shift();
        if (!e) break;
        log.cur = { text: e.text, kind: e.kind, pos: 0, el: makeLine(e) };
      }
      const c = log.cur;
      const step = instant ? c.text.length : Math.max(1, Math.ceil(c.text.length / 4)); // ~120ms full decode
      c.pos = Math.min(c.text.length, c.pos + step);
      renderCur(c);
      if (c.pos >= c.text.length) { log.cur = null; if (!instant) break; }
      else break;
    }
  }

  // ---------------------------------------------------------- battle mechanics
  function buildHpCells() {
    hpCellsEl.innerHTML = '';
    for (let i = 0; i < 16; i++) {
      const d = document.createElement('div');
      d.className = 'hpc';
      hpCellsEl.appendChild(d);
    }
  }
  function updateHp() {
    const frac = run.hp / run.maxHp;
    const lit = Math.max(run.hp > 0 ? 1 : 0, Math.round(frac * 16));
    [...hpCellsEl.children].forEach((c, i) => c.classList.toggle('on', i < lit));
    hpCellsEl.classList.toggle('mid', frac <= 0.5 && frac > 0.2);
    hpCellsEl.classList.toggle('low', frac <= 0.2);
  }
  function updateXpBar(provisionalXp) {
    const xp = provisionalXp != null ? provisionalXp : save.xp + (run ? run.lineXP + run.cheerXP : 0);
    const lv = levelForXp(xp);
    const lo = xpForLevel(lv), hi = xpForLevel(lv + 1);
    xpFillEl.style.width = clamp(((xp - lo) / (hi - lo)) * 100, 0, 100) + '%';
  }
  function updateCombo(pop) {
    comboEl.textContent = 'x' + run.combo;
    comboEl.classList.toggle('on', run.combo > 0);
    if (pop) { comboEl.classList.remove('pop'); void comboEl.offsetWidth; comboEl.classList.add('pop'); }
  }
  function dmgNum(x, y, text, cls) {
    const s = document.createElement('span');
    s.className = 'dmg' + (cls ? ' ' + cls : '');
    s.textContent = text;
    s.style.left = x + 'px';
    s.style.top = y + 'px';
    fxLayer.appendChild(s);
    after(700, () => s.remove());
  }
  function cheerBurst() {
    const b = document.createElement('span');
    b.className = 'cheer-burst';
    b.style.left = '38px';
    b.style.top = '56px';
    fxLayer.appendChild(b);
    after(450, () => b.remove());
  }
  function heroLunge() {
    const t = now();
    if (run.lastLungeAt && t - run.lastLungeAt < 120) return;
    run.lastLungeAt = t;
    heroSpriteEl.classList.remove('lunge');
    void heroSpriteEl.offsetWidth;
    heroSpriteEl.classList.add('lunge');
  }
  function shakeScreen() {
    screenInner.classList.remove('shake');
    void screenInner.offsetWidth;
    screenInner.classList.add('shake');
  }
  function flashOnce() {
    flashOverlay.classList.remove('one', 'three');
    void flashOverlay.offsetWidth;
    flashOverlay.classList.add('one');
  }

  function awardLineXp(base) {
    const t = now();
    run.xpTimes = run.xpTimes.filter((x) => t - x < 1000);
    if (run.xpTimes.length >= 10) return 0;                  // rate cap: `yes` can't farm
    run.xpTimes.push(t);
    let amt = base;
    if (run.frenzyUntil && t < run.frenzyUntil) amt *= 2;    // FRENZY doubles ticks
    run.lineXP += amt;
    return amt;
  }

  function hit(n, glance) {
    run.dmgTaken += n;
    if (run.laststand) run.overkill += n;                    // LAST STAND: overkill tally, never kill the process
    else if (run.hp - n <= 0) { run.hp = 1; run.laststand = true; laststandEl.classList.add('on'); }
    else run.hp -= n;
    updateHp();
    if (glance) { dmgNum(30, 44, '-' + n, 'gold'); return; }
    dmgNum(30, 42, '-' + n);
    const t = now();
    if (t - (run.lastHitFx || 0) > 150) {                    // debounce bursts
      run.lastHitFx = t;
      shakeScreen();
      hurtOverlay.classList.remove('on');
      void hurtOverlay.offsetWidth;
      hurtOverlay.classList.add('on');
    }
  }

  function startBattle(command, bossName) {
    clearFx();
    runToken += 1;
    const lv = levelForXp(save.xp);
    run = {
      token: runToken, command, bossName: bossName || null,
      boss: bossFor(command, bossName),
      hp: maxHpForLevel(lv), maxHp: maxHpForLevel(lv),
      stdout: 0, lineXP: 0, cheerXP: 0, dmgTaken: 0,
      stderrCount: 0, warnCount: 0, flawlessBroken: false,
      overkill: 0, laststand: false,
      combo: 0, frenzyUntil: 0,
      startAt: now(), lastLineAt: now(), lastOutAt: 0,
      xpTimes: [], errTail: [],
      charging: false, paused: false,
      fled: false, fleeing: false, ended: false, duration: 0,
      lastHitFx: 0, lastLungeAt: 0,
      warded: getBoss(command).warded
    };
    phase = 'run';
    resetBattleUi();
    show('battle');

    bossNameEl.textContent = `${run.boss.name} LV.${run.boss.lv}`;
    applySprite(bossSpriteEl, run.boss.cells, 2);
    applySprite(heroSpriteEl, heroCells, 2);
    playerTag.textContent = `CODER LV ${lv}`;
    turnEl.textContent = 'TURN 000';
    updateHp();
    updateXpBar();
    updateCombo(false);

    sysLine(`A WILD ${run.boss.name} DRAWS NEAR!`);
    if (run.warded) sysLine('THE WARD HOLDS. STDERR IS TAMED.');
    enqueueLog('> ' + command, 'dim');

    const myToken = runToken;
    invoke('start_quest', { command }).catch((err) => {
      if (!run || run.token !== myToken) return;
      run.ended = true;
      sysLine('THE GATE IS SEALED: ' + String(err));
      sysLine('PRESS B TO RETREAT.');
    });
  }

  function resetBattleUi() {
    log.queue = []; log.cur = null; log.overflowed = false;
    logLinesEl.innerHTML = '';
    logboxEl.classList.remove('frenzy', 'paused');
    fleeMeterEl.style.width = '0%';
    laststandEl.classList.remove('on');
    hurtOverlay.classList.remove('on');
    flashOverlay.classList.remove('one', 'three');
    comboEl.classList.remove('on', 'gold', 'pop');
    bossSpriteEl.classList.remove('dissolve', 'charging');
    bossWrap.classList.remove('boss-step');
    heroSpriteEl.style.opacity = '1';
    fxLayer.innerHTML = '';
    victoryEl.classList.remove('on');
    stampEl.classList.remove('in', 'parked');
    tallyEl.classList.remove('on');
    tallyRows.innerHTML = '';
    tallyMenu.classList.remove('on');
    defeatEl.classList.remove('on');
    screens.battle.classList.remove('defeat-mode');
    fatalLines.innerHTML = '';
  }

  // ---------------------------------------------------------- event handlers (registered before any start_quest)
  function onOutput(p) {
    if (state !== 'battle' || !run || run.ended || phase !== 'run') return;   // guard late events
    if (!p || typeof p.line !== 'string') return;
    const t = now();
    run.lastLineAt = t;
    if (run.charging) { run.charging = false; bossSpriteEl.classList.remove('charging'); }

    const raw = p.line;
    const isErrStream = p.stream === 'err';
    const errish = isErrStream || /error/i.test(raw);
    const warnish = !errish && /warn|warning|deprecat/i.test(raw);
    let kind = 'out';

    let crit = false;
    if (p.stream === 'out') {
      run.stdout += 1;
      run.lastOutAt = t;
      turnEl.textContent = 'TURN ' + String(run.stdout).padStart(3, '0');
      crit = run.stdout % 25 === 0;
      if (crit) { flashOnce(); dmgNum(168, 14, 'CRIT!', 'gold'); }
      heroLunge();
    }
    awardLineXp(crit ? 4 : 2);          // every output line grants XP (rate-capped)
    updateXpBar();

    if (isErrStream) {
      run.stderrCount += 1;
      run.errTail.push(clean(raw));
      if (run.errTail.length > 3) run.errTail.shift();
    }

    if (isErrStream && run.warded) {           // WARDED: stderr is neutral narration
      kind = 'ward';
      run.flawlessBroken = true;
    } else if (errish) {                       // stderr or /error/i -> real wound
      kind = 'err';
      run.flawlessBroken = true;
      hit(4);
    } else if (warnish) {                      // glancing blow
      kind = 'warn';
      run.warnCount += 1;
      run.flawlessBroken = true;
      hit(1, true);
    }

    enqueueLog(raw, kind);
  }

  function onDone(p) {
    if (!run || run.token !== runToken || run.ended) return;
    run.ended = true;
    run.duration = (now() - run.startAt) / 1000;
    if (state !== 'battle') {                                  // left the screen; settle silently
      if (run.fled) settleFlee(true);
      return;
    }
    const success = !!(p && p.success);
    if (run.fled) { settleFlee(false); return; }
    if (success) victorySequence(p);
    else defeatSequence(p);
  }

  // ---------------------------------------------------------- flee
  let fleeHold = null;
  function beginFleeHold() {
    if (!run || run.ended || phase !== 'run' || fleeHold) return;
    const t0 = now();
    fleeHold = setInterval(() => {
      if (!run || run.ended || phase !== 'run') return endFleeHold();
      const w = Math.min(1, (now() - t0) / 600);
      fleeMeterEl.style.width = (w * 100) + '%';
      if (w >= 1) { endFleeHold(); doFlee(); }
    }, 30);
  }
  function endFleeHold() {
    if (fleeHold) { clearInterval(fleeHold); fleeHold = null; }
    if (run && !run.fled) fleeMeterEl.style.width = '0%';
  }
  function doFlee() {
    if (!run || run.fleeing || run.ended) return;
    run.fleeing = true;
    run.fled = true;
    fleeMeterEl.style.width = '100%';
    sysLine('YOU TURN AND RUN!');
    bossWrap.classList.add('boss-step');       // the boss steps once toward the fleeing hero
    invoke('abort_quest').catch(() => {});
    const myToken = runToken;
    after(2500, () => {                        // safety net if done never lands
      if (run && run.token === myToken && !run.ended) {
        run.ended = true;
        run.duration = (now() - run.startAt) / 1000;
        settleFlee(state !== 'battle');
      }
    });
  }
  function settleFlee(silent) {
    const kept = Math.floor((run.lineXP + run.cheerXP) / 2);   // keep half of earned line-XP
    save.xp += kept;                                           // streak unchanged, no defeat increment
    save.lastFled = { kept };
    persist();
    if (!silent) after(500, () => enterSelect());
  }

  // ---------------------------------------------------------- cheer crits
  function tryCheer() {
    if (!run || run.ended || phase !== 'run') return;
    const t = now();
    if (run.lastOutAt && t - run.lastOutAt <= 200) {
      const bonus = run.frenzyUntil && t < run.frenzyUntil ? 2 : 1;
      run.cheerXP += bonus;
      run.combo += 1;
      updateCombo(true);
      cheerBurst();
      updateXpBar();
      if (run.combo >= 10 && !run.frenzyUntil) {
        run.frenzyUntil = t + 3000;
        logboxEl.classList.add('frenzy');
        comboEl.classList.add('gold');
        sysLine('FRENZY! THE RUNES BURN GOLD!');
      }
    } else if (run.combo > 0) {
      run.combo = 0;
      updateCombo(false);
    }
  }

  // ---------------------------------------------------------- charging tell (silence heartbeat)
  setInterval(() => {
    if (state !== 'battle' || !run || run.ended || phase !== 'run' || run.charging) return;
    if (now() - run.lastLineAt > 10000) {
      run.charging = true;
      bossSpriteEl.classList.add('charging');
      sysLine('THE FOE IS CHARGING POWER...');
    }
  }, 1000);

  // log processor heartbeat
  setInterval(logTick, 30);

  // ---------------------------------------------------------- victory
  let tallyReady = false;
  function victorySequence(payload) {
    phase = 'victory';
    tallyReady = false;
    endFleeHold();

    // ---- commit records first (crash-safe)
    const cmd = run.command;
    const b = touchBoss(cmd);
    const priorTimes = b.times.slice();
    const priorMedian = median(priorTimes);
    const priorBest = bestOf(priorTimes);
    const wasRevenge = b.losses > 0;
    const dur = run.duration;
    const lineTotal = run.lineXP + run.cheerXP;

    let total = 50 + lineTotal;
    const bonusRows = [];
    if (!run.flawlessBroken && run.stderrCount === 0 && run.warnCount === 0) { bonusRows.push(['FLAWLESS', '+50', 'bonus']); total += 50; }
    if (priorMedian != null && dur < priorMedian) { bonusRows.push(['SWIFT', '+30', 'bonus']); total += 30; }
    if (wasRevenge) { bonusRows.push(['REVENGE', '+25', 'revenge']); total += 25; }
    const sb = Math.min(50, 5 * save.streak);
    if (sb > 0) { bonusRows.push([`STREAK x${save.streak}`, '+' + sb, 'bonus']); total += sb; }

    const oldXp = save.xp;
    b.wins += 1;
    b.cleared = true;
    b.times.push(Math.round(dur * 10) / 10);
    if (b.times.length > 15) b.times.shift();
    save.streak += 1;
    save.xp += total;
    persist();

    // ---- ceremony
    bossSpriteEl.classList.add('dissolve');
    flashOverlay.classList.remove('one', 'three');
    void flashOverlay.offsetWidth;
    flashOverlay.classList.add('three');
    victoryEl.classList.add('on');

    after(850, () => {
      stampEl.classList.add('in');
      thudStamp();
      after(320, flashOnce);   // effects stay on-screen: the device never moves
    });

    after(1900, () => {
      stampEl.classList.add('parked');
      tallyEl.classList.add('on');
      const rows = [
        ['TURNS', String(run.stdout)],
        ['DMG TAKEN', String(run.dmgTaken)],
        ['TIME', fmtTime(dur) + (priorBest != null ? ` <span class="sub">B:${fmtTime(priorBest)}</span>` : '')],
        ['LINE EXP', '+' + lineTotal, 'bonus'],
        ...bonusRows,
        ['TOTAL EXP', '0', 'total']
      ];
      const els = rows.map(([k, v, cls]) => {
        const r = document.createElement('div');
        r.className = 'trow' + (cls ? ' ' + cls : '');
        r.innerHTML = `<span>${k}</span><span class="tv">${v}</span>`;
        tallyRows.appendChild(r);
        return r;
      });
      els.forEach((r, i) => after(280 * i, () => r.classList.add('on')));
      const totalIdx = els.length - 1;
      after(280 * totalIdx, () => {
        // odometer roll on the total
        const tv = els[totalIdx].querySelector('.tv');
        const t0 = now();
        const iv = setInterval(() => {
          const f = Math.min(1, (now() - t0) / 700);
          tv.textContent = '+' + Math.floor(total * f);
          if (f >= 1) clearInterval(iv);
        }, 40);
        fxTimers.add(iv);   // reuse clearFx (clearTimeout on an interval id is harmless; also clear properly)
        after(720, () => clearInterval(iv));
        animateXpFill(oldXp, save.xp, () => {
          tallyMenu.classList.add('on');
          tallyReady = true;
          updateSaveBlock();
          playerTag.textContent = 'CODER LV ' + levelForXp(save.xp);
        });
      });
    });
  }

  // animate the XP bar; interrupt into level-up when a threshold is crossed
  function animateXpFill(fromXp, toXp, onDone2) {
    const startLv = levelForXp(fromXp);
    const t0 = now();
    const durMs = 900;
    const iv = setInterval(() => {
      if (state !== 'battle' && state !== 'levelup') { clearInterval(iv); return; }
      if (state === 'levelup') return;   // paused while the jingle screen is up
      const f = Math.min(1, (now() - t0) / durMs);
      const cur = fromXp + (toXp - fromXp) * f;
      updateXpBar(cur);
      const lv = levelForXp(cur);
      if (lv > startLv) {
        clearInterval(iv);
        showLevelUp(startLv, levelForXp(toXp), () => {
          updateXpBar(toXp);
          onDone2();
        });
        return;
      }
      if (f >= 1) { clearInterval(iv); onDone2(); }
    }, 30);
  }

  let lvlDismiss = null;
  function showLevelUp(oldLv, newLv, onBack) {
    show('levelup');
    applySprite(lvlHero, heroCells, 2);
    lvlRows.innerHTML = '';
    const oldT = titleForLevel(oldLv), newT = titleForLevel(newLv);
    const rows = [
      [`LV ${oldLv} > ${newLv}`, 'gold'],
      [`MAX HP +${2 * (newLv - oldLv)}`, ''],
      ...(oldT !== newT ? [[oldT, ''], ['> ' + newT + ' <', 'sky']] : [])
    ];
    rows.forEach(([txt, cls], i) => {
      const r = document.createElement('div');
      r.className = 'lrow' + (cls ? ' ' + cls : '');
      r.textContent = txt;
      lvlRows.appendChild(r);
      after(300 * (i + 1), () => r.classList.add('on'));
    });
    jingleLevelUp();
    lvlDismiss = () => {
      lvlDismiss = null;
      show('battle');           // back to the victory tally
      onBack();
    };
  }

  // ---------------------------------------------------------- defeat
  let defeatSel = 0;
  function defeatSequence(payload) {
    phase = 'defeat';
    defeatSel = 0;
    endFleeHold();

    // records: no XP loss — earned line-XP is kept; streak resets; defeat counter arms revenge
    const b = touchBoss(run.command);
    b.losses += 1;
    save.streak = 0;
    save.xp += run.lineXP + run.cheerXP;
    persist();
    updateSaveBlock();

    applySprite(heroSpriteEl, heroFallenCells, 2);   // 2-frame collapse
    after(260, () => { if (phase === 'defeat') applySprite(heroSpriteEl, heroFallenCells, 2); });
    screens.battle.classList.add('defeat-mode');

    after(900, () => {
      defeatEl.classList.add('on');
      fatalLines.innerHTML = '';
      const tail = run.errTail.length ? run.errTail : ['(THE BLOW LEFT NO STDERR MARK)'];
      for (const l of tail) {
        const d = document.createElement('div');
        d.className = 'fl';
        d.textContent = fitLine(l, 31);
        fatalLines.appendChild(d);
      }
      const ex = document.createElement('div');
      ex.className = 'fl exit';
      ex.textContent = 'FOE: EXIT CODE ' + (payload && payload.code != null ? payload.code : '?');
      fatalLines.appendChild(ex);
      if (run.overkill > 0) {
        const ok = document.createElement('div');
        ok.className = 'fl mist';
        ok.textContent = 'OVERKILL x' + run.overkill;
        fatalLines.appendChild(ok);
      }
      renderDefeatMenu();
    });
  }
  function renderDefeatMenu() {
    dmRetry.classList.toggle('sel', defeatSel === 0);
    dmFlee.classList.toggle('sel', defeatSel === 1);
  }
  dmRetry.addEventListener('click', () => { if (phase === 'defeat') startBattle(run.command, run.bossName); });
  dmFlee.addEventListener('click', () => { if (phase === 'defeat') enterSelect(); });

  // ---------------------------------------------------------- input
  const KEYMAP = {
    ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    KeyD: 'a', KeyS: 'b', Enter: 'start', NumpadEnter: 'start',
    ShiftLeft: 'select', ShiftRight: 'select',
    KeyA: 'l', KeyF: 'r'
  };
  let paletteTimer = null;

  function onButton(btn, down, repeat) {
    // reflect on the physical shell
    if (!repeat) document.querySelectorAll(`[data-btn="${btn}"]`).forEach((el) => el.classList.toggle('pressed', down));

    if (!down) {   // releases
      if (btn === 'select') {
        if (paletteTimer) { clearTimeout(paletteTimer); paletteTimer = null; }
        paletteRow.classList.remove('on');
        endFleeHold();
      }
      return;
    }
    if (!repeat) audio();   // unlock WebAudio on first real input

    if (state === 'boot') {
      if (!repeat && !cart) konamiFeed(btn);   // the halted no-cart boot screen hides a secret
      return;
    }

    if (state === 'contra') {
      if (repeat || !cg) return;
      if (cg.over) {
        if (now() - (cg.overAt || 0) < 700) return;   // let GAME OVER land before mashed input acts on it
        if (btn === 'start' || btn === 'a') return enterContra();
        if (btn === 'b' || btn === 'select') return exitContra();
        return;
      }
      if (btn === 'select') return exitContra();
      if (btn === 'start') { cg.paused = !cg.paused; return; }
      if (btn === 'b') return cgJump();
      if (btn === 'a') return contraFire();
      return;
    }

    if (state === 'title') {
      if (repeat) return;
      if (btn === 'start' || btn === 'a') return (cart && cart.mode === 'quiz') ? enterMenu() : enterSelect();
      if (btn === 'select') { paletteTimer = setTimeout(() => paletteRow.classList.add('on'), 300); }
      return;
    }

    if (state === 'menu') {
      if (repeat) return;
      const save = loadQuizSave();
      if (btn === 'up' || btn === 'down') { if (save) { menuSel = 1 - menuSel; renderMenuSel(); } return; }
      if (btn === 'a' || btn === 'start') {
        if (menuSel === 0 && save) return enterHero(save);
        if (menuSel === 1) { clearQuizSave(); qQueue = []; return enterHero(null); }
        return;
      }
      if (btn === 'b') return enterTitle();
      return;
    }

    if (state === 'hero') {
      if (repeat) return;
      if (btn === 'up') { heroRow = (heroRow + 3) % 4; return renderHero(); }
      if (btn === 'down') { heroRow = (heroRow + 1) % 4; return renderHero(); }
      if (btn === 'left') return heroAdjust(-1);
      if (btn === 'right') return heroAdjust(1);
      if (btn === 'a' || btn === 'start') {
        if (heroRow === 3) return heroBegin();
        return heroAdjust(1);
      }
      if (btn === 'b') return enterMenu();
      return;
    }

    if (state === 'quiz') {
      if (quiz && quiz.over) { if (!repeat) enterMenu(); return; }
      if (quiz && quiz.waitIv && btn === 'a' && !repeat) {   // hop along the journey
        const th = $('travel-hero');
        th.classList.remove('hop'); void th.offsetWidth; th.classList.add('hop');
        return;
      }
      if (!quiz || quiz.locked) return;
      if (btn === 'up') { quiz.sel = (quiz.sel + quiz.cur.choices.length - 1) % quiz.cur.choices.length; return renderQuizSel(); }
      if (btn === 'down') { quiz.sel = (quiz.sel + 1) % quiz.cur.choices.length; return renderQuizSel(); }
      if (repeat) return;
      if (btn === 'a') return confirmAnswer();
      if (btn === 'b') { saveQuizRun(); stopQuizTimer(); return enterMenu(); }   // suspend the run
      return;
    }

    if (state === 'select') {
      if (btn === 'up') return moveSel(-1);
      if (btn === 'down') return moveSel(1);
      if (btn === 'l') return moveSel(-4);   // shoulder buttons page the menu
      if (btn === 'r') return moveSel(4);
      if (repeat) return;
      if (btn === 'a' || btn === 'start') return activateRow();
      if (btn === 'b') return enterTitle();
      if (btn === 'select') return toggleWard();
      return;
    }

    if (state === 'levelup') {
      if (!repeat && lvlDismiss) lvlDismiss();   // any button returns to the tally
      return;
    }

    if (state === 'battle') {
      if (phase === 'run') {
        if (repeat) return;
        if (btn === 'a') return tryCheer();                       // held A also fast-forwards (held.a)
        if (btn === 'b') {                                        // B = abort (flee) per contract
          if (run && run.ended) return enterSelect();             // failed-to-start escape hatch
          return doFlee();
        }
        if (btn === 'start') {                                    // pause log scroll; process keeps running
          if (run) { run.paused = !run.paused; logboxEl.classList.toggle('paused', run.paused); }
          return;
        }
        if (btn === 'select') return beginFleeHold();             // 600ms FLEE meter
        return;
      }
      if (phase === 'victory') {
        if (repeat || !tallyReady) return;
        if (btn === 'a' || btn === 'b') return enterSelect();
        if (btn === 'start') return startBattle(run.command, run.bossName);
        return;
      }
      if (phase === 'defeat') {
        if (repeat) return;
        if (btn === 'left' || btn === 'up') { defeatSel = 0; return renderDefeatMenu(); }
        if (btn === 'right' || btn === 'down') { defeatSel = 1; return renderDefeatMenu(); }
        if (btn === 'a' || btn === 'start') {
          if (defeatSel === 0) return startBattle(run.command, run.bossName);
          return enterSelect();
        }
        if (btn === 'b') return enterSelect();
      }
    }
  }

  window.addEventListener('keydown', (e) => {
    // typing inside the custom-quest input: game keys stand down
    if (document.activeElement === cmdInput) {
      if (e.key === 'Enter') { e.preventDefault(); launchCustom(); }
      else if (e.key === 'Escape') { cmdInput.blur(); moveSel(1); }
      else if (e.key === 'ArrowDown') { e.preventDefault(); moveSel(1); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); moveSel(-1); }
      return;
    }
    if (e.key === 'p' || e.key === 'P') { setPower(!powered); return; }
    if (e.key === 'c' || e.key === 'C') { if (trayOpen) closeTray(); else openTray(); return; }
    if (trayOpen && (e.key === 'Escape' || e.code === 'KeyS')) { closeTray(); return; }
    // no power gate needed: state 'off' matches no dispatch block in onButton,
    // so buttons depress visually but do nothing — like real hardware
    const btn = KEYMAP[e.code];
    if (!btn) return;
    e.preventDefault();
    if (e.repeat) { held[btn] = true; onButton(btn, true, true); return; }   // re-register a key held across a blur
    if (held[btn]) return;
    held[btn] = true;
    onButton(btn, true, false);
  });
  window.addEventListener('keyup', (e) => {
    const btn = KEYMAP[e.code];
    if (!btn || !held[btn]) return;
    held[btn] = false;
    onButton(btn, false, false);
  });
  window.addEventListener('blur', () => {
    for (const b in held) if (held[b]) { held[b] = false; onButton(b, false, false); }
    if (state === 'contra' && cg && !cg.over) cg.paused = true;   // don't drain lives while unfocused
  });

  // clickable shell buttons (press-and-hold works: A fast-forward, SELECT flee meter)
  document.querySelectorAll('[data-btn]').forEach((el) => {
    const btn = el.getAttribute('data-btn');
    const press = (e) => { e.preventDefault(); if (held[btn]) return; held[btn] = true; onButton(btn, true, false); };
    const release = () => { if (!held[btn]) return; held[btn] = false; onButton(btn, false, false); };
    el.addEventListener('pointerdown', press);
    el.addEventListener('pointerup', release);
    el.addEventListener('pointerleave', release);
    el.addEventListener('pointercancel', release);
  });

  cmdInput.addEventListener('focus', () => { if (state === 'select' && sel !== 0) { sel = 0; renderMenu(); } });
  cmdInput.addEventListener('input', () => cmdInput.classList.toggle('empty', !cmdInput.value));

  // ---------------------------------------------------------- init
  async function init() {
    fit();
    buildStarfield();
    buildPaletteRow();
    buildHpCells();
    updateSaveBlock();
    // listeners BEFORE any start_quest can fire
    try {
      await listenTo('quest://output', (e) => onOutput(e.payload));
      await listenTo('quest://done', (e) => onDone(e.payload));
    } catch (err) { /* no event system: demo shim already handles */ }
    // cartridges are local git repos: restore the cache, revalidate the inserted one
    try {
      carts = (JSON.parse(localStorage.getItem('cqa-repo-carts')) || []).filter((c) => c && typeof c.path === 'string');
    } catch (err) { carts = []; }
    const savedPath = localStorage.getItem('cqa-cart-id');
    if (savedPath) {
      try {
        cart = await invoke('load_cartridge', { path: savedPath });
        upsertCache(cart);
      } catch (err) {
        cart = null;
        localStorage.removeItem('cqa-cart-id');
        dropCache(savedPath);
      }
    }
    renderCartBack();
    if (cart && cart.mode === 'quiz') {
      const save = loadQuizSave();
      fetchBatch(save ? save.lv : 1, 6);         // generate through boot and title
    }
    ready = true;
    if (!powered) show('off');                   // wait for the power switch (unless one raced init)
  }
  init();

})();
