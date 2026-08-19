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

  const HERO_COLORS = { o: C.ink, h: C.gold, b: C.royal, g: C.gold, s: C.parch, c: C.plum };
  const HERO_MAP = [
    '................',
    '.....oooooo.....',
    '....ohhhhhho....',
    '...ohhhhhhhho...',
    '...ohhhhhhhho...',
    '...ohhhhhhhho...',
    '....ohhhhhho....',
    '..ooocbbbbcooo..',
    '.ocbbbbbbbbbbco.',
    '.ocbbbbbbbbbbco.',
    '.ocbbbbbbbbbbco.',
    '..obbbbbbbbbbo..',
    '..oggggggggggo..',
    '...obbo..obbo...',
    '...obbo..obbo...',
    '...oooo..oooo...'
  ];
  const HERO_FALLEN_MAP = [
    '................', '................', '................', '................',
    '................', '................', '................', '................',
    '................', '.....oooooo.....',
    '...oohhhhhhoo...',
    '..ohhhhhhhhhho..',
    '.ocbbbbbbbbbbco.',
    '.obbbbbbbbbbbbo.',
    '..oooooooooooo..',
    '................'
  ];
  const heroCells = cellsFromMap(HERO_MAP, HERO_COLORS);
  const heroFallenCells = cellsFromMap(HERO_FALLEN_MAP, HERO_COLORS);

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
  const shellEl = $('shell'), screenInner = $('screen-inner');
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

  const screens = { title: $('scr-title'), select: $('scr-select'), battle: $('scr-battle'), levelup: $('scr-levelup') };

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
    const s = Math.min(window.innerWidth / 584, window.innerHeight / 334);
    const snapped = s >= 1 ? Math.floor(s * 2) / 2 : Math.max(0.35, s); // half-integer steps, integer look
    shellEl.style.transform = `scale(${snapped})`;
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
  function enterTitle() {
    clearFx();
    updateSaveBlock();
    show('title');
  }

  // ---------------------------------------------------------- quest select
  let menu = [];      // [{type:'custom'}|{type:'quest',quest}|{type:'recent',cmd}]
  let sel = 0;
  let typing = null;  // typewriter interval

  function enterSelect() {
    clearFx();
    show('select');
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
      after(320, () => {
        flashOnce();
        shellEl.classList.remove('nudge');     // the ONE full-shell reaction
        void shellEl.offsetWidth;
        shellEl.classList.add('nudge');
      });
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
    KeyX: 'a', KeyZ: 'b', Enter: 'start', NumpadEnter: 'start',
    ShiftLeft: 'select', ShiftRight: 'select'
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

    if (state === 'title') {
      if (repeat) return;
      if (btn === 'start' || btn === 'a') return enterSelect();
      if (btn === 'select') { paletteTimer = setTimeout(() => paletteRow.classList.add('on'), 300); }
      return;
    }

    if (state === 'select') {
      if (btn === 'up') return moveSel(-1);
      if (btn === 'down') return moveSel(1);
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
    const btn = KEYMAP[e.code];
    if (!btn) return;
    e.preventDefault();
    if (e.repeat) { onButton(btn, true, true); return; }
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
    try {
      const qs = await invoke('list_quests');
      quests = Array.isArray(qs) ? qs.filter((q) => q && typeof q.command === 'string') : [];
    } catch (err) { quests = []; }
    ready = true;
    enterTitle();
  }
  init();

})();
