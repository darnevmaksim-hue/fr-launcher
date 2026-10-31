// ===== Диагностика инстанса: дерево зависимостей + авторемонт =====
// Вкладка «Установленные» (Моды): две кнопки.
//  «Дерево зависимостей» — что от чего зависит, что сломается при удалении,
//   отсутствующие required-зависимости и конфликты (IPC deps:analyze).
//  «Проверить и исправить» — битые jar, дубликаты, недостающие зависимости
//   с кнопкой «Устранить» (IPC repair:scan / repair:run).
(function () {
  const API = window.API;
  if (!API || !API.depsAnalyze) return;
  const btnDeps = document.getElementById('btn-deps-tree');
  const btnScan = document.getElementById('btn-repair-scan');
  const treeBox = document.getElementById('deps-tree');
  const outBox = document.getElementById('repair-out');
  const stateLbl = document.getElementById('repair-state');
  if (!btnDeps || !btnScan || !treeBox || !outBox) return;

  const q = (s, c) => (c || document).querySelector(s);
  const qa = (s, c) => Array.from((c || document).querySelectorAll(s));
  function esc(s) {
    return String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // state объявлен в renderer.js — классические скрипты делят глобальную область
  function currentInstance() {
    try {
      if (typeof state !== 'undefined' && state.selectedInstance && Array.isArray(state.instances)) {
        const inst = state.instances.find((i) => i.id === state.selectedInstance);
        if (inst) return inst;
      }
    } catch (e) {}
    return null;
  }
  function needInstance() {
    const inst = currentInstance();
    if (!inst) {
      outBox.hidden = false;
      treeBox.hidden = true;
      outBox.innerHTML = '<div class="repair-hint">Сначала выбери модпак — селект «Активный модпак» сверху страницы.</div>';
      return null;
    }
    return inst;
  }
  function busy(on, text) {
    btnDeps.disabled = on;
    btnScan.disabled = on;
    if (stateLbl) stateLbl.textContent = on ? (text || 'Работаю...') : '';
  }
  function tryRefreshInstalled() {
    try { if (typeof refreshInstalledMods === 'function') refreshInstalledMods(); } catch (e) {}
  }

  // ---------- Игра упала: подсказка «что делать» ----------
  // main шлёт game:crash при выходе с ненулевым кодом: переключаемся на
  // «Моды → Установленные» и сами запускаем проверку, чтобы игрок сразу увидел
  // битые файлы/дубликаты/недостающие зависимости, а не гадал, что сломало игру.
  if (API.onGameCrash) {
    API.onGameCrash((d) => {
      const code = d && d.code != null ? d.code : '?';
      try {
        if (typeof switchPageTo === 'function') switchPageTo(document.getElementById('page-mods'));
      } catch (e) {}
      try {
        const tab = document.querySelector('.mtab[data-tab="installed"]');
        if (tab) tab.click();
      } catch (e) {}
      if (typeof flash === 'function') flash('Игра завершилась с ошибкой (код ' + code + ') — проверяю моды');
      try { btnScan.click(); } catch (e) {}
    });
  }

  // ---------- Дерево знаний зависимостей ----------
  // Вид как в React Flow, но сверху вниз: сверху главный мод, дуга со
  // стрелкой спускается вниз к модам, которые без него не работают.
  // Карточка мода: монограмма, название, файл и статус (в сборке/встроена/...).
  let flowRoots = null; // дерево для отрисовки (для перерисовки при сворачивании)
  const collapsedKeys = new Set(); // свёрнутые ветви (ключ узла в дереве)

  function flowCard(node, key, depth) {
    const kids = node.children || [];
    const isEmb = node.state === 'embedded';
    let st = 'installed';
    let stTxt = '✓ в сборке';
    if (isEmb) { st = 'embedded'; stTxt = '◆ встроена'; }
    else if (node.type === 'optional' || node.type === 'recommended') { st = 'optional'; stTxt = '◇ опционально'; }
    else if (node.type === 'incompatible') { st = 'conflict'; stTxt = '⚠ конфликт'; }
    const sub = node.file || node.slug || '';
    const letter = esc(String(node.title || node.slug || '?').slice(0, 1).toUpperCase());
    return '<div class="mf-card dt-card' + (isEmb ? ' emb' : '') + (depth === 0 ? ' root' : '') + '"' +
      ' data-k="' + esc(key) + '" data-has="' + (kids.length ? 1 : 0) + '"' +
      ' title="' + esc(node.title || node.slug) + (node.file ? ' · ' + esc(node.file) : '') +
      (kids.length ? ' · клик — свернуть/развернуть ветвь' : '') + '">' +
      '<div class="mf-top"><span class="mf-ico">' + letter + '</span>' +
      '<span class="mf-names"><strong>' + esc(node.title || node.slug) + '</strong>' +
      '<span>' + esc(sub) + '</span></span>' +
      (kids.length ? '<span class="mf-tw">' + (collapsedKeys.has(key) ? '▸' : '▾') + '</span>' : '') +
      '</div><div class="mf-status ' + st + '">' + stTxt + '</div></div>';
  }

  // раскладка сверху вниз: сверху главный мод, ниже — то, что без него не
  // заработает. Строка одной «пачки» детей не резиновая: как только она
  // упирается в бюджет ширины, дети переносятся на следующую строку вниз (иначе
  // у Fabric API с 200+ зависимыми получалась стена в 50 000 пикселей вбок).
  // Корневые деревья упаковываются в полосы тем же бюджетом. Карточки
  // absolutely + SVG-дуги bezier со стрелкой вниз.
  function drawFlow() {
    if (!canvasEl || !flowRoots) return;
    const CARD_W = 200, CARD_H = 62, H_GAP = 36, V_GAP = 56, PAD = 24, GROUP_GAP = 44;
    // два бюджета ширины: внутри ветви детей переносим по первому, а сами
    // корневые деревья упаковываем в полосы по более широкому второму — так
    // дерево выходит компактным и по ширине, и по высоте
    const vw = (vpEl && vpEl.clientWidth) || 900;
    const wrapBudget = Math.max(1400, Math.round(vw * 2.4));
    const packBudget = Math.max(2600, Math.round(vw * 5));
    const flat = [];
    const links = []; // {from: rec, to: key}
    const byKey = new Map();

    // 1. меряем блоки: сколько места займёт узел вместе со своими потомками
    function measure(n, key) {
      const kids = collapsedKeys.has(key) ? [] : (n.children || []);
      if (!kids.length) return { n: n, key: key, w: CARD_W, h: CARD_H, rows: [] };
      const infos = kids.map((k, j) => measure(k, key + '/' + j));
      const rows = [];
      let cur = [];
      let curW = 0;
      let rowH = 0;
      let blockW = 0;
      for (const c of infos) {
        if (cur.length && curW + c.w > wrapBudget) {
          rows.push({ items: cur, h: rowH });
          blockW = Math.max(blockW, curW - H_GAP);
          cur = [];
          curW = 0;
          rowH = 0;
        }
        cur.push(c);
        curW += c.w + H_GAP;
        if (c.h > rowH) rowH = c.h;
      }
      if (cur.length) {
        rows.push({ items: cur, h: rowH });
        blockW = Math.max(blockW, curW - H_GAP);
      }
      let h = CARD_H + V_GAP; // своя карточка + отступ до первой строки детей
      rows.forEach((r, i) => { h += r.h + (i ? V_GAP : 0); });
      return { n: n, key: key, w: Math.max(CARD_W, blockW), h: h, rows: rows };
    }

    // 2. расставляем: карточка узла — по центру своего блока, дети — строками под ней
    function place(m, left, top, depth) {
      const rec = { n: m.n, key: m.key, depth: depth, y: top, xMid: left + m.w / 2 };
      flat.push(rec);
      byKey.set(m.key, rec);
      let y = top + CARD_H + V_GAP;
      for (const r of m.rows) {
        let x = left;
        for (const c of r.items) {
          links.push({ from: rec, to: c.key });
          place(c, x, y, depth + 1);
          x += c.w + H_GAP;
        }
        y += r.h + V_GAP;
      }
      return rec;
    }

    // 3. полосы: корневые деревья друг за другом, перенос — по budget упаковки
    const groups = flowRoots.map((r, i) => {
      const m = measure(r, 'r' + i);
      const gi = flat.length;
      place(m, 0, 0, 0);
      return { from: gi, nodes: flat.length - gi, width: m.w, height: m.h, ox: 0, oy: 0 };
    });
    let rowX = 0, rowY = 0, rowH = 0, maxRowW = 0;
    for (const g of groups) {
      if (rowX > 0 && rowX + g.width > packBudget) {
        maxRowW = Math.max(maxRowW, rowX - GROUP_GAP);
        rowY += rowH + GROUP_GAP;
        rowX = 0;
        rowH = 0;
      }
      g.ox = rowX;
      g.oy = rowY;
      rowX += g.width + GROUP_GAP;
      if (g.height > rowH) rowH = g.height;
    }
    maxRowW = Math.max(maxRowW, rowX > 0 ? rowX - GROUP_GAP : 0);
    for (const g of groups) {
      const to = g.from + g.nodes;
      for (let j = g.from; j < to; j++) {
        flat[j].xMid += g.ox;
        flat[j].y += g.oy;
      }
    }
    const W = PAD * 2 + Math.max(0, maxRowW);
    const H = PAD * 2 + rowY + rowH;
    canvasEl.style.width = W + 'px';
    canvasEl.style.height = H + 'px';
    canvasEl.innerHTML = flat.map((f) => flowCard(f.n, f.key, f.depth).replace(
      '<div class="mf-card',
      '<div style="left:' + Math.round(PAD + f.xMid - CARD_W / 2) + 'px;top:' + Math.round(PAD + f.y) + 'px" class="mf-card')).join('');
    // координаты карточек — по фактической высоте DOM (перенос названия)
    const geo = new Map();
    canvasEl.querySelectorAll('.mf-card').forEach((el) => {
      geo.set(el.getAttribute('data-k'), {
        x: parseFloat(el.style.left), y: parseFloat(el.style.top), h: el.offsetHeight,
      });
    });
    let svg = '<svg class="mf-svg" width="' + W + '" height="' + H + '">' +
      '<defs><marker id="mfar" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" ' +
      'orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 Z" fill="context-stroke"/></marker></defs>';
    for (const ln of links) {
      const a = geo.get(ln.from.key);
      const b = geo.get(ln.to);
      if (!a || !b) continue;
      const x1 = a.x + CARD_W / 2;
      const y1 = a.y + a.h;
      const x2 = b.x + CARD_W / 2;
      const y2 = b.y;
      const dy = Math.max(24, (y2 - y1) * 0.45);
      const emb = (byKey.get(ln.to) || {}).n && byKey.get(ln.to).n.state === 'embedded';
      svg += '<path class="mf-edge' + (emb ? ' emb' : '') + '" d="M' + x1 + ',' + y1 +
        ' C' + x1 + ',' + (y1 + dy) + ' ' + x2 + ',' + (y2 - dy) + ' ' + x2 + ',' + y2 +
        '" marker-end="url(#mfar)"/>';
    }
    svg += '</svg>';
    canvasEl.insertAdjacentHTML('afterbegin', svg);
  }

  // ---------- вид дерева: масштаб колесом, перетаскивание зажатой ЛКМ ----------
  // Без скроллбаров: колесо приближает к точке под курсором, зажатая ЛКМ
  // двигает полотно; кнопки в правом верхнем углу — + / − и «вписать».
  const view = { x: 0, y: 0, k: 1 };
  let vpEl = null; // .dtree (viewport)
  let canvasEl = null; // .dtree-canvas (полотно)

  function applyView() {
    if (canvasEl) {
      canvasEl.style.transform = 'translate(' + view.x + 'px,' + view.y + 'px) scale(' + view.k + ')';
    }
  }
  function fitView() {
    if (!vpEl || !canvasEl) return;
    const vw = vpEl.clientWidth;
    const vh = vpEl.clientHeight;
    const cw = canvasEl.offsetWidth; // незатронутые transform размеры
    const ch = canvasEl.offsetHeight;
    if (!vw || !vh || !cw || !ch) return;
    const pad = 20;
    let k = Math.min((vw - pad * 2) / cw, (vh - pad * 2) / ch, 1);
    if (k < 0.12) k = 0.12; // как и у колеса: ниже не мельчим, иначе ничего не разобрать
    view.k = k;
    view.x = (vw - cw * k) / 2;
    view.y = pad;
    applyView();
  }
  function zoomCenter(factor) {
    if (!vpEl) return;
    zoomAt(vpEl.clientWidth / 2, vpEl.clientHeight / 2, factor);
  }
  function zoomAt(px, py, factor) {
    const k2 = Math.min(2.5, Math.max(0.12, view.k * factor));
    const s = k2 / view.k;
    view.x = px - (px - view.x) * s;
    view.y = py - (py - view.y) * s;
    view.k = k2;
    applyView();
  }
  treeBox.addEventListener('wheel', (ev) => {
    if (!vpEl || !canvasEl) return;
    const r = vpEl.getBoundingClientRect();
    if (ev.clientX < r.left || ev.clientX > r.right || ev.clientY < r.top || ev.clientY > r.bottom) return;
    ev.preventDefault();
    zoomAt(ev.clientX - r.left, ev.clientY - r.top, Math.exp(-ev.deltaY * 0.0016));
  }, { passive: false });

  // перетаскивание полотна; если двигали — это не клик по карточке
  let pan = null;
  let suppressClick = false;
  treeBox.addEventListener('pointerdown', (ev) => {
    if (ev.button !== 0 || !vpEl) return;
    if (!ev.target.closest || !ev.target.closest('.dtree')) return;
    if (ev.target.closest('.dtree-ctl')) return;
    suppressClick = false;
    pan = { x0: ev.clientX, y0: ev.clientY, vx0: view.x, vy0: view.y, moved: false };
  });
  window.addEventListener('pointermove', (ev) => {
    if (!pan) return;
    const dx = ev.clientX - pan.x0;
    const dy = ev.clientY - pan.y0;
    if (!pan.moved && Math.abs(dx) + Math.abs(dy) > 4) {
      pan.moved = true;
      if (vpEl) vpEl.classList.add('dt-panning');
    }
    if (pan.moved) {
      view.x = pan.vx0 + dx;
      view.y = pan.vy0 + dy;
      applyView();
    }
  });
  function endPan() {
    if (!pan) return;
    if (pan.moved) {
      suppressClick = true;
      // клик после перетаскивания прилетает сразу за pointerup; если он придёт
      // мимо дерева (отпустили за его пределами), флаг не должен «съесть»
      // следующий нормальный клик по карточке
      setTimeout(() => { suppressClick = false; }, 0);
    }
    if (vpEl) vpEl.classList.remove('dt-panning');
    pan = null;
  }
  window.addEventListener('pointerup', endPan);
  window.addEventListener('pointercancel', endPan);

  // клик по карточке: кнопки вида ±/вписать, иначе свернуть/развернуть ветвь
  treeBox.addEventListener('click', (ev) => {
    if (suppressClick) { suppressClick = false; return; }
    const zbtn = ev.target && ev.target.closest ? ev.target.closest('.dtc-btn') : null;
    if (zbtn && treeBox.contains(zbtn)) {
      const z = zbtn.getAttribute('data-z');
      if (z === 'fit') fitView();
      else zoomCenter(z === 'in' ? 1.35 : 1 / 1.35);
      return;
    }
    const card = ev.target && ev.target.closest ? ev.target.closest('.mf-card') : null;
    if (!card || !treeBox.contains(card)) return;
    if (card.getAttribute('data-has') !== '1') return;
    const key = card.getAttribute('data-k');
    if (collapsedKeys.has(key)) collapsedKeys.delete(key);
    else collapsedKeys.add(key);
    drawFlow(); // перерисовка без сброса вида (view не трогаем)
  });

  function renderTree(res) {
    if (!res.ok) {
      treeBox.innerHTML = '<div class="repair-hint">Ошибка: ' + esc(res.error) + '</div>';
      return;
    }
    const total = res.installed || (res.local && res.local.scanned) || res.totalCount || 0;
    let html = '<div class="deps-head">Дерево знаний зависимостей · ' + total + ' модов в сборке · ' +
      esc(res.mcVersion) + ' / ' + esc(res.loader) + '</div>';
    if (res.local) {
      html += '<div class="repair-hint">Локальная проверка самих jar: встроенных библиотек (JarJar): ' +
        res.local.embedded + ', не заявленных в сборке зависимостей: <b>' + res.local.missing + '</b></div>';
    }

    if (res.problems && res.problems.length) {
      html += '<div class="metric-analysis deps-problems">Проблем: <b>' + res.problems.length + '</b>' +
        res.problems.map((p) => '<span class="ma-hint">' + (p.kind === 'missing-required'
          ? '▸ нет «' + esc(p.title || p.dep) + '» — нужна моду «' + esc(p.parent) + '», игра может не стартовать'
          : '▸ «' + esc(p.dep) + '» конфликтует с «' + esc(p.parent) + '»') + '</span>').join('') +
        '<span class="ma-hint">Недостающее чинится во вкладке «Проверить и исправить».</span></div>';
    }

    const roots = res.roots || [];
    if (roots.length) {
      html += '<div class="dtree"><div class="dtree-canvas"></div>' +
        '<div class="dtree-ctl">' +
        '<button class="dtc-btn" data-z="in" title="Приблизить">+</button>' +
        '<button class="dtc-btn" data-z="out" title="Отдалить">−</button>' +
        '<button class="dtc-btn" data-z="fit" title="Вписать дерево">⌂</button>' +
        '</div></div>';
      html += '<div class="repair-hint">Показаны только моды этой сборки. Ветвь = «без мода-родителя этот мод не работает»: ' +
        'сверху главный мод, ниже — то, что без него сломается. ' +
        'Колесо мыши — масштаб к курсору, зажатая ЛКМ — перетаскивание, клик по карточке — свернуть ветвь, ⌂ — вписать дерево. ' +
        'В ветвях: ' + (res.shownCount || 0) + ' модов, без связей: ' + (res.noDepsCount || 0) +
        ' (списком ниже). Всё недостающее — в блоке «Проблем» и во вкладке «Проверить и исправить».</div>';
    } else {
      html += '<div class="repair-hint">Ни от одного мода в сборке никто не зависит. Чисто.</div>';
    }

    // моды, не попавшие ни в одну ветвь: ни от кого не зависят и никто не
    // зависит от них — показываем списком, чтобы были видны ВСЕ моды сборки
    const idle = res.idle || [];
    if (idle.length) {
      const noMeta = idle.filter((m) => m.note).length;
      html += '<div class="dt-idle"><div class="dt-idle-head">Модов без связей: <b>' + idle.length +
        '</b>' + (noMeta ? ' <span class="dt-nometa">(в т.ч. ' + noMeta + ' jar без метаданных мода — наведи, чтобы увидеть файл)</span>' : '') +
        ' <span class="dt-idle-sub">(в дерево не попадают: ни от кого не зависят и никто не зависит от них)</span></div>' +
        '<div class="dt-idle-list">' +
        idle.map((m) => '<span class="dt-chip' + (m.note ? ' warn' : '') + '" title="' +
          esc(m.file) + (m.note ? ' — ' + esc(m.note) : '') + '">' + esc(m.title) + '</span>').join('') +
        '</div></div>';
    }
    treeBox.innerHTML = html;
    vpEl = treeBox.querySelector('.dtree');
    canvasEl = treeBox.querySelector('.dtree-canvas');
    flowRoots = roots.length ? roots : null;
    collapsedKeys.clear();
    if (vpEl && canvasEl) {
      drawFlow();
      requestAnimationFrame(fitView);
    }
  }

  btnDeps.addEventListener('click', async () => {
    const inst = needInstance();
    if (!inst) return;
    const showTree = treeBox.hidden;
    treeBox.hidden = !showTree;
    if (showTree) {
      outBox.hidden = true;
      treeBox.innerHTML = '<div class="repair-hint">Анализирую зависимости на Modrinth...</div>';
      busy(true, 'Анализ зависимостей...');
      try {
        const res = await API.depsAnalyze({ instanceId: inst.id, mcVersion: inst.mcVersion, loader: inst.loader });
        renderTree(res || { ok: false, error: 'нет ответа' });
      } catch (e) {
        treeBox.innerHTML = '<div class="repair-hint">Ошибка: ' + esc(e.message) + '</div>';
      }
      busy(false);
    }
  });

  // ---------- Проверка и ремонт ----------
  let lastIssues = [];

  function renderScan(res) {
    outBox.hidden = false;
    treeBox.hidden = true;
    if (!res.ok) {
      outBox.innerHTML = '<div class="repair-hint">Ошибка: ' + esc(res.error) + '</div>';
      return;
    }
    lastIssues = res.issues || [];
    if (!lastIssues.length) {
      outBox.innerHTML = '<div class="repair-hint ok">Проблем не найдено: битых файлов нет, дубликатов нет, все required-зависимости на месте (' + res.modCount + ' модов).</div>';
      return;
    }
    const rows = lastIssues.map((it, i) =>
      '<label class="issue-row"><input type="checkbox" class="issue-cb" data-i="' + i + '" checked>' +
      '<span class="issue-title">' + esc(it.title) + '</span></label>').join('');
    outBox.innerHTML =
      '<div class="repair-head">Найдено проблем: <b>' + lastIssues.length + '</b></div>' +
      '<div class="issue-list">' + rows + '</div>' +
      '<div class="repair-actions">' +
      '<button class="btn-primary" id="btn-repair-fix">Устранить отмеченные</button>' +
      '<button class="btn-ghost" id="btn-repair-refresh">Проверить заново</button>' +
      '</div>';
    q('#btn-repair-refresh').addEventListener('click', () => btnScan.click());
    q('#btn-repair-fix').addEventListener('click', fixSelected);
  }

  async function fixSelected() {
    const inst = currentInstance();
    if (!inst) return;
    const chosen = qa('.issue-cb').filter((cb) => cb.checked).map((cb) => lastIssues[parseInt(cb.dataset.i, 10)]).filter(Boolean);
    if (!chosen.length) { if (stateLbl) stateLbl.textContent = 'Ничего не отмечено'; return; }
    busy(true, 'Устраняю...');
    try {
      const res = await API.repairRun({ instanceId: inst.id, issues: chosen });
      if (res && res.ok) {
        const lines = [];
        if (res.done && res.done.length) lines.push('<div class="repair-done">✓ ' + res.done.map(esc).join('<br>✓ ') + '</div>');
        if (res.errors && res.errors.length) lines.push('<div class="repair-err">✕ ' + res.errors.map(esc).join('<br>✕ ') + '</div>');
        outBox.innerHTML = '<div class="repair-head">Готово: устранено ' + (res.done ? res.done.length : 0) + '</div>' + lines.join('');
        tryRefreshInstalled();
        if (typeof flash === 'function') flash('Ремонт: устранено ' + (res.done ? res.done.length : 0));
      } else {
        outBox.innerHTML = '<div class="repair-hint">Ошибка: ' + esc(res && res.error) + '</div>';
      }
    } catch (e) {
      outBox.innerHTML = '<div class="repair-hint">Ошибка: ' + esc(e.message) + '</div>';
    }
    busy(false);
  }

  btnScan.addEventListener('click', async () => {
    const inst = needInstance();
    if (!inst) return;
    outBox.hidden = false;
    treeBox.hidden = true;
    outBox.innerHTML = '<div class="repair-hint">Проверяю файлы модов и зависимости...</div>';
    busy(true, 'Проверка инстанса...');
    try {
      const res = await API.repairScan({ instanceId: inst.id });
      renderScan(res || { ok: false, error: 'нет ответа' });
    } catch (e) {
      outBox.innerHTML = '<div class="repair-hint">Ошибка: ' + esc(e.message) + '</div>';
    }
    busy(false);
  });
})();
