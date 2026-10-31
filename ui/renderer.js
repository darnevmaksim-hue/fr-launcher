const $ = (sel, ctx) => (ctx || document).querySelector(sel);
const $$ = (sel, ctx) => (ctx || document).querySelectorAll(sel);

const state = {
  settings: { java: '', ram: 4096, mcDir: '' },
  mcVersions: [],
  instances: [],
  selectedInstance: null,
  selectedLoader: 'vanilla',
  selectedMc: '1.20.1',
  javaPath: '',
  installedMods: [],
  importBtn: null, // кнопка «Импорт» в каталоге — во время импорта показывает %
};

// ===== Navigation =====
const PAGE_ANIM_CLS = ['leaving', 'entering', 'enter-fwd', 'enter-bwd', 'leave-fwd', 'leave-bwd'];

function clearPageAnim(el) { PAGE_ANIM_CLS.forEach((c) => el.classList.remove(c)); }

// переход между вкладками со сдвигом влево/вправо (как в iOS)
function switchPageTo(nextEl) {
  if (!nextEl) return;
  const cur = document.querySelector('.page.active');
  if (cur === nextEl) return;

  const order = Array.from(document.querySelectorAll('.nav-item')).map((b) => b.dataset.page).filter(Boolean);
  const ci = cur ? order.indexOf((cur.id || '').replace('page-', '')) : -1;
  const ni = order.indexOf((nextEl.id || '').replace('page-', ''));
  const dir = ni >= ci ? 'fwd' : 'bwd';
  const content = document.querySelector('.content');
  // сброс скролла без анимации — иначе виден «уезд» контента
  if (content) content.scrollTop = 0;
  // на время перехода фиксируем переполнение контейнера: иначе браузер
  // пересчитывает layout/скроллбар на каждом кадре и анимация дёргается
  if (content) content.classList.add('animating');

  const done = () => { if (content) content.classList.remove('animating'); };

  if (cur) {
    clearPageAnim(cur);
    cur.classList.remove('active');
    cur.classList.add('leaving', dir === 'fwd' ? 'leave-fwd' : 'leave-bwd');
    const onCurEnd = (e) => { if (e.target === cur) { cur.removeEventListener('animationend', onCurEnd); clearPageAnim(cur); } };
    cur.addEventListener('animationend', onCurEnd);
    setTimeout(() => { if (cur.classList.contains('leaving')) { cur.removeEventListener('animationend', onCurEnd); clearPageAnim(cur); } }, 700);
  }

  clearPageAnim(nextEl);
  nextEl.classList.add('active', 'entering', dir === 'fwd' ? 'enter-fwd' : 'enter-bwd');
  const onEnd = (e) => { if (e.target === nextEl) { nextEl.removeEventListener('animationend', onEnd); clearPageAnim(nextEl); done(); } };
  nextEl.addEventListener('animationend', onEnd);
  setTimeout(() => { if (nextEl.classList.contains('entering')) { nextEl.removeEventListener('animationend', onEnd); clearPageAnim(nextEl); done(); } }, 700);
}

// Тяжёлые рендеры страницы запускаем ПОСЛЕ старта анимации, иначе первый
// кадр перехода блокируется синхронным построением DOM и переход «залипает».
function refreshPageSoon(page) {
  requestAnimationFrame(() => requestAnimationFrame(() => {
    if (page === 'account') renderAccount();
    if (page === 'mods') renderInstances();
    if (page === 'link') renderLaunchers();
  }));
}

$$('.nav-item').forEach((btn) => {
  btn.addEventListener('click', () => {
    $$('.nav-item').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    switchPageTo($('#page-' + btn.dataset.page));
    refreshPageSoon(btn.dataset.page);
  });
});

// ===== App info =====
(async () => {
  const info = await window.API.appInfo();
  $('#app-ver').textContent = info.version;
})();

// ===== Автообновление =====
const updateOverlay = $('#update-overlay');
const updateBtn = $('#update-btn');
const updateStatus = $('#update-status');
let pendingUpdate = null;
let updateInstallerPath = null;

function escMd(md) {
  return String(md || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
function renderChangelog(md) {
  // примитивный список из обычного текста/маркдауна
  const lines = String(md || '').split(/\r?\n/);
  let html = '';
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) { html += ''; continue; }
    if (/^#{1,6}\s/.test(line)) html += `<div class="upd-h">${escMd(line.replace(/^#{1,6}\s*/, ''))}</div>`;
    else if (/^[-*+]\s+/.test(line)) html += `<div class="upd-li">• ${escMd(line.replace(/^[-*+]\s+/, ''))}</div>`;
    else if (/^\d+[.)]\s+/.test(line)) html += `<div class="upd-li">${escMd(line)}</div>`;
    else html += `<div class="upd-p">${escMd(line)}</div>`;
  }
  return html || 'Обновление вышло — обнови лаунчер, чтобы получить новинки.';
}

function showUpdateModal(upd) {
  pendingUpdate = upd;
  $('#update-new-ver').textContent = upd.version;
  $('#update-changelog').innerHTML = renderChangelog(upd.changelog);
  updateBtn.textContent = 'Обновить';
  updateStatus.textContent = '';
  updateBtn.disabled = false;
  updateBtn.classList.remove('hidden');
  $('#update-later').classList.remove('hidden');
  updateOverlay.classList.remove('hidden');
}

async function checkForUpdates(silent) {
  const upd = await window.API.updateCheck();
  if (!upd) { if (!silent) flash('Обновлений нет — у тебя актуальная версия'); return; }
  showUpdateModal(upd);
}

updateBtn.addEventListener('click', async () => {
  if (!pendingUpdate) return;
  updateBtn.disabled = true;
  updateBtn.textContent = 'Скачиваю...';
  updateStatus.textContent = '';
  if (!pendingUpdate.downloadUrl) {
    updateStatus.textContent = 'Файл обновления недоступен. Загрузи вручную: ' + pendingUpdate.releaseUrl;
    updateBtn.disabled = false;
    updateBtn.textContent = 'Обновить';
    return;
  }
  const dl = await window.API.updateDownload(pendingUpdate.downloadUrl);
  if (!dl || !dl.ok) {
    updateStatus.textContent = 'Ошибка скачивания: ' + ((dl && dl.error) || 'нет сети').replace(/^HTTPS? /i, '');
    updateBtn.disabled = false;
    updateBtn.textContent = 'Повторить';
    return;
  }
  updateInstallerPath = dl.path;
  updateBtn.textContent = 'Обновляю...';
  updateStatus.textContent = 'Скачано. Лаунчер закроется, обновится и запустится заново.';
  const inst = await window.API.updateInstall(dl.path);
  if (!inst || !inst.ok) {
    updateStatus.textContent = 'Ошибка обновления: ' + ((inst && inst.error) || '');
    updateBtn.disabled = false;
    updateBtn.textContent = 'Повторить';
  }
});
$('#update-later').addEventListener('click', async () => {
  if (pendingUpdate) await window.API.updateSkip(pendingUpdate.version);
  updateOverlay.classList.add('hidden');
});
$('#update-close').addEventListener('click', () => updateOverlay.classList.add('hidden'));

// проверяем обновление при старте (незаметно)
setTimeout(() => { checkForUpdates(true); }, 3500);

// ===== Quick launch (home) =====
function renderQuickGrid() {
  const grid = $('#quick-grid');
  if (!grid) return;
  if (!state.instances.length) {
    grid.innerHTML = `<div class="quick-card" data-new="1"><div class="quick-ico">+</div><div class="quick-body"><div class="quick-name">Создать модпак</div><div class="quick-meta">Начни свою первую сборку</div></div><span class="quick-go">→</span></div>`;
    return;
  }
  grid.innerHTML = state.instances
    .slice(0, 6)
    .map(
      (i) => `<div class="quick-card" data-inst="${i.id}">
        <div class="quick-ico">${(i.name[0] || '▤').toUpperCase()}</div>
        <div class="quick-body"><div class="quick-name">${i.name}</div><div class="quick-meta">${i.loader} • ${i.mcVersion}</div></div>
        <span class="quick-go">▶</span>
      </div>`
    )
    .join('');
}
function wireQuickGrid() {
  const grid = $('#quick-grid');
  if (!grid) return;
  grid.querySelectorAll('.quick-card').forEach((c) => {
    c.addEventListener('click', () => {
      const instId = c.dataset.inst;
      if (instId) {
        state.selectedInstance = instId;
        renderInstances();
        renderModPackBar();
        refreshInstalledMods();
        gotoPage('home');
      } else if (c.dataset.new) {
        $('#btn-new-instance').click();
      }
    });
  });
}

// Метка: переход на страницу через навигацию
function gotoPage(page) {
  const btn = document.querySelector(`.nav-item[data-page="${page}"]`);
  if (btn) btn.click();
  else switchPageTo($('#page-' + page));
}

// Банер рекламы сайта
const SITE_URL = 'https://darnevmaksim-hue.github.io/ballisticys-site/#downloads';
(() => {
  const b = $('#promo-banner');
  if (b) b.addEventListener('click', (e) => { e.preventDefault(); window.API.openExternal(SITE_URL); });
})();

// Топбар-аватар ведёт на страницу аккаунта
$('#acct-avatar').addEventListener('click', () => gotoPage('account'));

// ===== Telemetry =====
let playtime = {};
function fmtPlaytime(seconds) {
  if (!seconds) return '0 ч';
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  if (h <= 0) return m + ' мин';
  return h + ' ч' + (m ? ' ' + m + ' мин' : '');
}
function renderTelemetry() {
  const stat = state.instances.length;
  let totalSec = 0, sessions = 0;
  Object.values(playtime).forEach((v) => { totalSec += (v.seconds || 0); sessions += (v.sessions || 0); });
  const data = [
    { title: 'Инстансов активно', val: String(stat), bar: Math.min(stat * 40, 100) },
    { title: 'Время в игре', val: fmtPlaytime(totalSec), bar: totalSec ? Math.min(totalSec / 3600 * 6, 100) : 2 },
    { title: 'Сессий сыграно', val: String(sessions), bar: sessions ? Math.min(sessions * 6, 100) : 2 },
  ];
  $('#telemetry-home').innerHTML = data
    .map((d) => `<div class="tele-card"><div class="t-title">${d.title}</div><div class="t-val">${d.val}</div><div class="t-bar"><span style="width:${d.bar}%"></span></div></div>`)
    .join('');
}

// ===== Settings =====
function loadSettings() {
  try { state.settings = { ...state.settings, ...JSON.parse(localStorage.getItem('fr_settings')) }; } catch (e) {}
  $('#set-java').value = state.settings.java || '';
  $('#set-ram').value = state.settings.ram || 4096;
  $('#set-mc-dir').value = state.settings.mcDir || '';
}
loadSettings();

$('#btn-save-settings').addEventListener('click', async () => {
  state.settings.java = $('#set-java').value.trim();
  state.settings.ram = parseInt($('#set-ram').value, 10) || 4096;
  state.settings.mcDir = $('#set-mc-dir').value.trim();
  localStorage.setItem('fr_settings', JSON.stringify(state.settings));
  state.javaPath = state.settings.java;
  flash('Настройки сохранены. Java: ' + (state.javaPath || 'авто'));
});

$('#btn-java-browse').addEventListener('click', async () => {
  const p = await window.API.openDialog({ properties: ['openFile'], filters: [{ name: 'Java', extensions: ['exe'] }] });
  if (p) { $('#set-java').value = p; state.javaPath = p; }
});

$('#btn-mcdir-browse').addEventListener('click', async () => {
  const p = await window.API.openDialog({ properties: ['openDirectory'] });
  if (p) $('#set-mc-dir').value = p;
});

function esc(s) {
  return String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function flash(msg) {
  const el = $('#conn-status');
  el.innerHTML = `<span class="dot"></span>${msg}`;
  setTimeout(() => (el.innerHTML = '<span class="dot"></span>SYSTEM READY'), 2500);
}

// ===== Account (offline ник + Microsoft) =====
const ACCT_KEY = 'fr_account';
let account = { username: '', mode: 'offline' };
let msDevice = null;
let msPollTimer = null;

function headFromSkin(skinB64, sizePx) {
  // Кроп лица из скина (64x32): область (8,8)-(16,16)*, фрейм 8x8
  return new Promise((resolve) => {
    if (!skinB64) return resolve('');
    const img = new Image();
    img.onload = () => {
      try {
        const c = document.createElement('canvas');
        c.width = sizePx; c.height = sizePx;
        const ctx = c.getContext('2d');
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, 8, 8, 8, 8, 0, 0, sizePx, sizePx);
        resolve(c.toDataURL('image/png'));
      } catch (e) { resolve(''); }
    };
    img.onerror = () => resolve('');
    img.src = skinB64;
  });
}

async function applyAvatar(acct) {
  const face = await headFromSkin(acct.skinB64, 64);
  const bigAv = $('#acct-big-avatar');
  if (face && (acct.mode === 'msa')) {
    if (bigAv) { bigAv.textContent = ''; bigAv.style.backgroundImage = 'url(' + face + ')'; bigAv.style.backgroundSize = 'cover'; }
    const top = $('#acct-avatar');
    if (top) { top.textContent = ''; top.style.backgroundImage = 'url(' + face + ')'; top.style.backgroundSize = 'cover'; }
  } else {
    const letter = acct.username ? acct.username[0].toUpperCase() : '?';
    if (bigAv) { bigAv.textContent = letter; bigAv.style.backgroundImage = ''; }
    const top = $('#acct-avatar');
    if (top) { top.textContent = letter; top.style.backgroundImage = ''; }
  }
}

async function loadAccount() {
  try { account = { ...account, ...JSON.parse(localStorage.getItem(ACCT_KEY)) }; } catch (e) {}
  // если есть сохранённый Microsoft-аккаунт — подтягиваем
  try {
    const ms = await window.API.msStatus();
    if (ms && ms.mode === 'msa' && ms.username) {
      account.username = ms.username;
      account.mode = 'msa';
      account.skinB64 = ms.skinB64 || '';
      localStorage.setItem(ACCT_KEY, JSON.stringify(account));
    }
  } catch (e) {}
  renderAccount();
  if (account.username) $('#sel-username').value = account.username;
}
loadAccount();
window.API.onAccountChanged(async (ms) => {
  if (ms && ms.mode === 'msa') {
    account.username = ms.username;
    account.mode = 'msa';
    account.skinB64 = ms.skinB64 || '';
    localStorage.setItem(ACCT_KEY, JSON.stringify(account));
  } else if (ms === null) {
    account.username = account.mode === 'msa' ? '' : account.username;
    account.skinB64 = '';
    if (account.mode === 'msa') account.mode = 'offline';
    localStorage.setItem(ACCT_KEY, JSON.stringify(account));
  }
  renderAccount();
  if (account.username) $('#sel-username').value = account.username;
  flash(ms ? ('Вошёл через Microsoft: ' + ms.username) : 'Вышел из Microsoft');
});
$('#sel-username').addEventListener('change', () => {
  const name = $('#sel-username').value.trim();
  if (name) { account.username = name; account.mode = 'offline'; localStorage.setItem(ACCT_KEY, JSON.stringify(account)); renderAccount(); }
});

function renderAccount() {
  const name = account.username || 'Не залогинен';
  const logged = !!account.username;
  const loggedMs = account.mode === 'msa' && logged;
  // страница аккаунта
  const bigAv = $('#acct-big-avatar');
  if (bigAv) bigAv.textContent = logged ? account.username[0].toUpperCase() : '?';
  const bigName = $('#acct-big-name');
  if (bigName) bigName.textContent = name;
  const bigMode = $('#acct-big-mode');
  if (bigMode) {
    bigMode.textContent = logged ? (loggedMs ? 'Microsoft' : 'offline') : 'offline';
    bigMode.style.color = loggedMs ? 'var(--mint)' : logged ? 'var(--amber)' : 'var(--dim)';
  }
  // блок Microsoft
  const idle = $('#ms-idle');
  const loggedBox = $('#ms-logged');
  if (idle) idle.style.display = loggedMs ? 'none' : 'block';
  if (loggedBox) loggedBox.style.display = loggedMs ? 'block' : 'none';
  if (loggedMs) {
    const nm = $('#ms-logged-name');
    if (nm) nm.textContent = account.username;
    const uid = $('#ms-logged-uuid');
    if (uid) uid.textContent = 'UUID: ' + (account.uuid || account.msUuid || '');
    const img = $('#ms-skin-big');
    if (img && account.skinB64) img.src = account.skinB64;
  }
  applyAvatar(account);
}

// Аккаунт: офлайн-ник
$('#btn-acct-offline').addEventListener('click', () => {
  const n = ($('#acct-offline-name').value || '').trim();
  if (!n) { flash('Введи игровое имя'); return; }
  account.username = n;
  account.mode = 'offline';
  localStorage.setItem(ACCT_KEY, JSON.stringify(account));
  renderAccount();
  const su = $('#sel-username');
  if (su) su.value = n;
  flash('Вошёл как ' + n);
});
$('#acct-offline-name').addEventListener('keydown', (e) => { if (e.key === 'Enter') $('#btn-acct-offline').click(); });

// Аккаунт: вкладки offline / microsoft
$$('.acct-tab').forEach((btn) => {
  btn.addEventListener('click', () => {
    $$('.acct-tab').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    $$('.acct-pane').forEach((p) => p.classList.toggle('active', p.dataset.apane === btn.dataset.atab));
  });
});

// ---- Microsoft: вход (device code flow, как в официальном лаунчере) ----
async function beginMsLogin() {
  if (msPollTimer) return;
  const err = $('#ms-err');
  if (err) err.classList.add('hidden');
  const flow = $('#ms-flow');
  const idle = $('#ms-idle');
  if (idle) idle.style.display = 'none';
  if (flow) flow.style.display = 'block';
  const st = $('#ms-flow-status');
  if (st) st.textContent = 'Получаю код...';
  const res = await window.API.msStart();
  if (!res || !res.ok) {
    if (st) st.textContent = 'Ошибка: ' + ((res && res.error) || 'не удалось получить код');
    if (idle) idle.style.display = 'block';
    return;
  }
  msDevice = res;
  const uc = $('#ms-usercode');
  if (uc) uc.textContent = res.userCode;
  if (st) st.textContent = 'Открыл страницу? Введи код ' + res.userCode + ' на сайте.';
  const open = $('#btn-ms-open');
  if (open) open.onclick = () => window.API.openExternal(res.verificationUri);
  window.API.openExternal(res.verificationUri);
  let pollEvery = (res.interval || 5) * 1000;
  const startPolling = () => {
    msPollTimer = setInterval(async () => {
      const p = await window.API.msPoll(msDevice.deviceCode);
      if (p.slowDown) {
        // сервер просит реже — перезапускаем таймер с большим интервалом
        clearInterval(msPollTimer); msPollTimer = null;
        pollEvery += 5000;
        startPolling();
        return;
      }
      if (p.status === 'done') {
      clearInterval(msPollTimer); msPollTimer = null;
      if (st) st.textContent = 'Подтверждено! Вхожу...';
      const auth = await window.API.msAuth(p);
      if (auth && auth.ok) {
        account.username = auth.account.username;
        account.mode = 'msa';
        account.uuid = auth.account.uuid;
        account.skinB64 = auth.account.skinB64 || '';
        localStorage.setItem(ACCT_KEY, JSON.stringify(account));
        if (flow) flow.style.display = 'none';
        renderAccount();
        const su = $('#sel-username');
        if (su) su.value = account.username;
        flash('✓ Вошёл через Microsoft: ' + account.username);
      } else {
        if (err) { err.textContent = 'Ошибка входа: ' + ((auth && auth.error) || 'неизвестная'); err.classList.remove('hidden'); }
        if (idle) idle.style.display = 'block';
        if (flow) flow.style.display = 'none';
      }
    } else if (p.status === 'denied' || p.status === 'expired' || p.status === 'error') {
      clearInterval(msPollTimer); msPollTimer = null;
      if (st) st.textContent = p.status === 'denied' ? 'Вход отменён.' : p.status === 'expired' ? 'Код истёк, попробуй снова.' : ('Ошибка: ' + (p.error || ''));
      if (idle) idle.style.display = 'block';
      if (flow) flow.style.display = 'none';
    }
    }, pollEvery);
  };
  startPolling();
}
$('#btn-acct-ms').addEventListener('click', beginMsLogin);
$('#btn-ms-cancel').addEventListener('click', () => {
  clearInterval(msPollTimer); msPollTimer = null;
  msDevice = null;
  const flow = $('#ms-flow'); if (flow) flow.style.display = 'none';
  const idle = $('#ms-idle'); if (idle) idle.style.display = 'block';
});
$('#btn-acct-ms-logout').addEventListener('click', async () => {
  await window.API.msLogout();
  account.username = '';
  account.skinB64 = '';
  account.uuid = '';
  account.mode = 'offline';
  localStorage.setItem(ACCT_KEY, JSON.stringify(account));
  renderAccount();
});

// ===== Java auto-detect =====
window.API.onJavaLog((d) => {
  const log = $('#game-log');
  if (log) log.innerHTML += (d.text || '').replace(/\</g, '&lt;') + '\n';
  log.scrollTop = log.scrollHeight;
});

async function initJava() {
  if (state.settings.java) { state.javaPath = state.settings.java; return; }
  const res = await window.API.ensureJava();
  if (res && res.error) {
    flash('Java 17+ не найден: ' + res.error + '. Укажи путь вручную.');
    state.javaPath = '';
  } else if (res && res.path) {
    state.javaPath = res.path;
    flash((res.system ? 'Java найден' : 'JDK 17 скачан') + ': ' + res.raw);
  } else {
    flash('Java 17+ не найден. Укажи путь вручную.');
  }
}

// ===== MC Versions =====
async function initMcVersions() {
  const v = await window.API.mcVersions();
  state.mcVersions = v;
  if (v.some((x) => x.id === '1.20.1')) state.selectedMc = '1.20.1';
  else if (v.length) state.selectedMc = v[0].id;
}

// ===== Instances =====
async function renderInstances() {
  state.instances = await window.API.instancesList();
  // если модпак не выбран или удалён — берём первый
  if (!state.selectedInstance || !state.instances.find((x) => x.id === state.selectedInstance)) {
    state.selectedInstance = state.instances.length ? state.instances[0].id : null;
  }
  const grid = $('#instance-grid');
  if (!state.instances.length) {
    grid.innerHTML = `<div class="mod-card"><p class="mc-desc">Инстансов пока нет. Создай первый.</p></div>`;
  } else {
    grid.innerHTML = state.instances
      .map(
        (i) => `<div class="instance-card${i.id === state.selectedInstance ? ' selected' : ''}" data-id="${i.id}">
          <button class="ic-del" data-del="${i.id}">✕</button>
          <div class="ic-name">${i.name}</div>
          <div class="ic-meta">${i.loader} • ${i.mcVersion} • модов: ${i.modsCount || 0}</div>
          <div class="ic-meta dim">в игре: ${fmtPlaytime((playtime[i.id]||{}).seconds||0)}</div>
        </div>`
      )
      .join('');
  }
  // select handler
  $$('.instance-card').forEach((c) => c.addEventListener('click', (e) => {
    if (e.target.classList.contains('ic-del')) return;
    state.selectedInstance = c.dataset.id;
    renderInstances();
    renderPlay();
    renderModPackBar();
    refreshInstalledMods();
    modsState.page = 1;
    loadMods();
  }));
  $$('.ic-del').forEach((b) => b.addEventListener('click', async (e) => {
    e.stopPropagation();
    await window.API.instanceDelete(b.dataset.del);
    await renderInstances();
    if (state.selectedInstance && !state.instances.find((x) => x.id === state.selectedInstance)) {
      state.selectedInstance = state.instances.length ? state.instances[0].id : null;
    }
    renderPlay();
    renderModPackBar();
    renderTelemetry();
  }));
  renderPlay();
  renderModPackBar();
  renderQuickGrid();
  wireQuickGrid();
}

$('#btn-new-instance').addEventListener('click', async () => {
  const data = await showCreateModpack();
  if (!data) return;
  const inst = await window.API.instanceCreate({ name: data.name, mcVersion: data.mcVersion, loader: data.loader, ram: state.settings.ram, javaPath: state.javaPath });
  state.selectedInstance = inst.id;
  await renderInstances();
  await refreshInstalledMods();
  renderTelemetry();
});

$('#btn-new-modpack').addEventListener('click', async () => {
  const data = await showCreateModpack();
  if (!data) return;
  const inst = await window.API.instanceCreate({ name: data.name, mcVersion: data.mcVersion, loader: data.loader, ram: state.settings.ram, javaPath: state.javaPath });
  state.selectedInstance = inst.id;
  await renderInstances();
  await refreshInstalledMods();
  renderTelemetry();
  loadMods();
  flash('Модпак создан: ' + data.name);
});

// Модалка создания модпака (как в TLauncher): имя + версия + загрузчик
async function showCreateModpack() {
  if (!state.mcVersions.length) state.mcVersions = await window.API.mcVersions();
  return new Promise((resolve) => {
    const wrap = document.createElement('div');
    wrap.style.cssText =
      'position:fixed;inset:0;z-index:999;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.6);backdrop-filter:blur(5px);font-family:var(--font-mono)';
    const loaders = ['Vanilla', 'Fabric', 'Forge', 'NeoForge'];
    const all = state.mcVersions || [];
    const oldTypes = new Set(['old_release', 'old_alpha', 'old_beta']);
    const ordered = [
      ...all.filter((v) => v.type === 'release' || !v.type),
      ...all.filter((v) => v.type === 'snapshot'),
      ...all.filter((v) => oldTypes.has(v.type)),
    ];
    const opts = ordered
      .map((v) => `<option value="${v.id}">${v.id}${v.type === 'snapshot' ? ' (снапшот)' : oldTypes.has(v.type) ? ' (старая)' : ''}</option>`)
      .join('');
    wrap.innerHTML = `
      <div style="background:var(--panel);border:1px solid var(--line2);border-radius:var(--radius-lg);padding:1.8rem;width:min(440px,94%);box-shadow:0 30px 70px rgba(0,0,0,.6)">
        <div style="color:var(--text);font-size:1.2rem;font-weight:700;margin-bottom:1.2rem">Создать модпак</div>

        <div style="color:var(--dim);font-size:0.75rem;margin-bottom:0.35rem">Название модпака</div>
        <input id="mp-name" value="My Modpack" placeholder="например: TechnoPack" style="width:100%;background:var(--glass);border:1px solid var(--stroke);border-radius:8px;color:var(--text);padding:0.7rem;font-family:var(--font-mono);margin-bottom:1rem;font-size:0.9rem">

        <div style="color:var(--dim);font-size:0.75rem;margin-bottom:0.35rem">Версия Minecraft</div>
        <select id="mp-version" style="width:100%;background-color:var(--panel3);border:1px solid var(--stroke);border-radius:8px;color:var(--text);padding:0.7rem;font-family:var(--font-mono);margin-bottom:1rem;font-size:0.9rem;color-scheme:dark">
          ${opts}
        </select>

        <div style="color:var(--dim);font-size:0.75rem;margin-bottom:0.4rem">Ядро (загрузчик)</div>
        <div id="mp-loaders" style="display:flex;gap:0.5rem;flex-wrap:wrap;margin-bottom:1.4rem"></div>

        <div style="display:flex;gap:0.5rem;justify-content:flex-end">
          <button data-v="cancel" style="padding:0.6rem 1.1rem;background:transparent;border:1px solid var(--stroke);color:var(--dim);border-radius:9px;cursor:pointer;font-family:var(--font-mono)">Отмена</button>
          <button data-v="ok" style="padding:0.65rem 1.4rem;background:linear-gradient(135deg,var(--accent),var(--cyan));color:#fff;border:none;border-radius:9px;cursor:pointer;font-family:var(--font-mono);font-weight:700">Создать</button>
        </div>
      </div>`;
    document.body.appendChild(wrap);

    // загрузчики
    const lrow = wrap.querySelector('#mp-loaders');
    let chosenLoader = state.selectedLoader || 'vanilla';
    loaders.forEach((name) => {
      const key = name === 'Vanilla' ? 'vanilla' : name;
      const b = document.createElement('button');
      b.textContent = name;
      b.dataset.l = key;
      b.style.cssText =
        `padding:0.5rem 1rem;border:1px solid var(--stroke);border-radius:9px;background:transparent;color:var(--dim);cursor:pointer;font-family:var(--font-mono);font-size:0.85rem;` +
        (key === chosenLoader ? `border-color:var(--accent);color:#fff;background:var(--acsoft);` : ``);
      b.addEventListener('click', () => {
        chosenLoader = key;
        [...lrow.children].forEach((x) => {
          x.style.borderColor = 'var(--stroke)';
          x.style.color = 'var(--dim)';
          x.style.background = 'transparent';
        });
        b.style.borderColor = 'var(--accent)';
        b.style.color = '#fff';
        b.style.background = 'var(--acsoft)';
      });
      lrow.appendChild(b);
    });

    const nameInput = wrap.querySelector('#mp-name');
    nameInput.focus();
    nameInput.select();
    const close = (v) => { wrap.remove(); resolve(v); };
    wrap.addEventListener('click', (e) => {
      const btn = e.target.closest('[data-v]');
      if (!btn) return;
      if (btn.dataset.v === 'ok') {
        const name = nameInput.value.trim();
        const ver = wrap.querySelector('#mp-version').value;
        if (name) { wrap.remove(); resolve({ name, mcVersion: ver, loader: chosenLoader }); }
        else nameInput.style.borderColor = 'var(--red)';
      } else close(null);
    });
    wrap.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') wrap.querySelector('[data-v="ok"]').click();
      if (e.key === 'Escape') close(null);
    });
  });
}

function syncLoadedInstance() {
  const i = state.instances.find((x) => x.id === state.selectedInstance);
  if (!i) return;
  const loaderMap = { Vanilla: 'vanilla', Fabric: 'Fabric', Forge: 'Forge', NeoForge: 'NeoForge' };
  const target = loaderMap[i.loader] || 'vanilla';
  state.selectedLoader = target;
  state.selectedMc = i.mcVersion;
}

function renderPlay() {
  // заполнить селект модпаков
  const sel = $('#sel-instance');
  sel.innerHTML = state.instances
    .map((i) => `<option value="${i.id}" ${i.id === state.selectedInstance ? 'selected' : ''}>${i.name} — ${i.loader} • ${i.mcVersion}</option>`)
    .join('');
  if (!state.instances.length) sel.innerHTML = `<option value="">Нет модпаков</option>`;

  $('#play-pack-hint').textContent = state.selectedInstance
    ? (() => {
        const i = state.instances.find((x) => x.id === state.selectedInstance);
        return i ? `Версия ${i.mcVersion} • Ядро ${i.loader} • Модов: ${i.modsCount || 0}` : '';
      })()
    : 'Создай или выбери модпак ниже';

  $('#play-instance').innerHTML = state.selectedInstance
    ? (() => {
        const i = state.instances.find((x) => x.id === state.selectedInstance);
        return i
          ? `<div class="pi-name">${i.name}</div><div class="pi-meta">${i.loader} • ${i.mcVersion}</div><div class="pi-meta">Модов: ${i.modsCount || 0}</div><div class="pi-meta dim">⏱ в игре: ${fmtPlaytime((playtime[i.id]||{}).seconds||0)} • сессий: ${(playtime[i.id]||{}).sessions||0}</div>`
          : `<div class="pi-meta">Выбери модпак</div>`;
      })()
    : `<div class="pi-meta">Создай или выбери модпак</div>`;
  syncLoadedInstance();
}

// выбор модпака из селекта на вкладке «Играть»
$('#sel-instance').addEventListener('change', () => {
  state.selectedInstance = $('#sel-instance').value || null;
  renderPlay();
  refreshInstalledMods();
  renderModPackBar();
  renderInstances(); // подсветить карточку на вкладке «Модпаки»
  modsState.page = 1;
  loadMods();
});

// ===== Launch =====
window.API.onGameLog((d) => {
  const log = $('#game-log');
  const text = d.text.replace(/</g, '&lt;');
  log.innerHTML += `<span class="${d.cls}">${text}</span>`;
  log.scrollTop = log.scrollHeight;
});

window.API.onProgress((d) => {
  renderProgress(d.done || 0, d.total || 0, d.label || '', d.detail || '');
});

function renderProgress(done, total, label, detail) {
  const prog = $('#prog');
  prog.hidden = false;
  const pct = total > 0 ? Math.max(0, Math.min(100, Math.round((done / total) * 100))) : null;
  $('#prog-fill').style.width = (pct === null ? 100 : pct) + '%';
  $('#prog-pct').textContent = pct === null ? '' : pct + '%';
  if (label) $('#prog-label').textContent = label;
  if (detail) $('#prog-detail').textContent = detail;
  showGlobalProgress(pct, label, detail);
  // кнопка «Импорт» в каталоге модпаков — показываем процент, иначе
  // пользователь минутами смотрит на «⏳ Импорт...» и думает, что ничего не идёт
  if (state.importBtn) {
    state.importBtn.textContent = pct === null ? '⏳ ...' : '⏳ ' + pct + '%';
  }
}

// Глобальный прогресс: импорт модпака запускается на вкладке «Моды»,
// а старый #prog живёт на «Главная» — оттуда его не видно
let gprogTimer = null;
function showGlobalProgress(pct, label, detail) {
  const g = $('#gprog');
  if (!g) return;
  g.hidden = false;
  $('#gprog-fill').style.width = (pct === null ? 100 : pct) + '%';
  $('#gprog-pct').textContent = pct === null ? '' : pct + '%';
  if (label) $('#gprog-label').textContent = label;
  if (detail) $('#gprog-detail').textContent = detail;
  if (gprogTimer) clearTimeout(gprogTimer);
  // закончили (100%) — прячем через секунду, чтобы увидеть «100%»
  if (pct !== null && pct >= 100) gprogTimer = setTimeout(hideProgress, 1400);
}

function hideProgress() {
  const prog = $('#prog');
  prog.hidden = true;
  $('#prog-fill').style.width = '0%';
  $('#prog-pct').textContent = '';
  const g = $('#gprog');
  if (g) g.hidden = true;
  if (gprogTimer) { clearTimeout(gprogTimer); gprogTimer = null; }
  state.importBtn = null;
}

$('#btn-play').addEventListener('click', async () => {
  const log = $('#game-log');
  log.innerHTML = '';
  renderProgress(0, 0, '');
  const inst = state.instances.find((x) => x.id === state.selectedInstance);
  if (!inst) { log.innerHTML = '<span class="err">// Выбери модпак</span>'; return; }
  if (!state.javaPath) {
    await initJava();
    if (!state.javaPath) { log.innerHTML = '<span class="err">// Java не найдена. Укажи в Настройках.</span>'; return; }
  }
  const loaderMap = { Vanilla: 'vanilla', Fabric: 'Fabric', Forge: 'Forge', NeoForge: 'NeoForge' };
  $('#btn-play').disabled = true;
  $('#btn-play').textContent = 'ЗАПУСК...';
  try {
    const res = await window.API.launch({
      mcVersion: inst.mcVersion,
      loader: loaderMap[inst.loader] || inst.loader,
      loaderVersion: inst.loaderVersion || null,
      javaPath: state.javaPath,
      ram: state.settings.ram,
      username: account.username || $('#sel-username').value.trim() || 'Player',
      modJars: [],
      instanceId: inst.id,
    });
    if (res && res.error) log.innerHTML += `<span class="err">// ${res.error.replace(/</g, '&lt;')}</span>`;
  } catch (e) {
    log.innerHTML += `<span class="err">// ${(e && e.message ? e.message : e).toString().replace(/</g, '&lt;')}</span>`;
  }
  $('#btn-play').disabled = false;
  $('#btn-play').textContent = 'ИГРАТЬ';
  hideProgress();
});

// ===== Доступ к файлам версии =====
const clearSel = () => {
  try { $('#sel-instance').value = ''; $('#sel-instance').dispatchEvent(new Event('change')); } catch (e) {}
};

$('#btn-open-gamedir').addEventListener('click', async () => {
  const id = state.selectedInstance;
  if (!id) { clearSel(); return; }
  const dir = await window.API.instanceDir(id);
  if (dir) window.API.openPath(dir);
});

$('#btn-open-mcdir').addEventListener('click', async () => {
  const dir = await window.API.mcDir();
  if (dir) window.API.openPath(dir);
});

// ===== Autofit: моды подстраиваются под активный модпак =====
function activeInstance() {
  return state.instances.find((x) => x.id === state.selectedInstance) || null;
}
// Версия MC активного модпака
function modMc() {
  const i = activeInstance();
  return (i && i.mcVersion) || state.selectedMc;
}
// Нормализованное ядро активного модпака (fabric/forge/neoforge/vanilla)
function modLoader() {
  const i = activeInstance();
  const l = (i && i.loader) || state.selectedLoader;
  return l === 'Vanilla' ? 'vanilla' : String(l).toLowerCase();
}

function renderModPackBar() {
  const sel = $('#mod-pack-select');
  const meta = $('#mod-pack-meta');
  sel.innerHTML = state.instances
    .map((i) => `<option value="${i.id}" ${i.id === state.selectedInstance ? 'selected' : ''}>${i.name}</option>`)
    .join('');
  if (!state.instances.length) {
    sel.innerHTML = `<option value="">Нет модпаков</option>`;
    meta.textContent = 'Создай модпак, чтобы каталог подстроился под него';
    return;
  }
  const i = activeInstance();
  meta.textContent = i
    ? `Версия ${i.mcVersion} • ${i.loader} — моды будут подобраны автоматически`
    : 'Выбери модпак';
}

$('#mod-pack-select').addEventListener('change', () => {
  state.selectedInstance = $('#mod-pack-select').value || null;
  // подхватываем версию и ядро модпака
  const i = activeInstance();
  if (i) {
    state.selectedMc = i.mcVersion;
    const loaderMap = { Vanilla: 'vanilla', Fabric: 'Fabric', Forge: 'Forge', NeoForge: 'NeoForge' };
    state.selectedLoader = loaderMap[i.loader] || 'vanilla';
  }
  renderPlay();
  renderModPackBar();
  refreshInstalledMods();
  renderInstances();
  modsState.page = 1;
  loadMods();
});

$('#btn-modpack-here').addEventListener('click', async () => {
  const data = await showCreateModpack();
  if (!data) return;
  const inst = await window.API.instanceCreate({ name: data.name, mcVersion: data.mcVersion, loader: data.loader, ram: state.settings.ram, javaPath: state.javaPath });
  state.selectedInstance = inst.id;
  state.selectedMc = inst.mcVersion;
  state.selectedLoader = inst.loader;
  await renderInstances();
  renderModPackBar();
  await refreshInstalledMods();
  renderTelemetry();
  modsState.page = 1;
  loadMods();
  flash('Модпак создан: ' + data.name);
});

// ===== Mods (Modrinth) =====
const MOD_PER_PAGE = 24;
const MOD_CATS = [
  { slug: '', label: '★ Топ', desc: 'самые популярные' },
  { slug: 'optimization', label: 'Оптимизация' },
  { slug: 'performance', label: 'Производительность' },
  { slug: 'hardcore', label: 'Хардкор' },
  { slug: 'magic', label: 'Магия' },
  { slug: 'technology', label: 'Технологии' },
  { slug: 'adventure', label: 'Приключения' },
  { slug: 'decoration', label: 'Декор' },
  { slug: 'library', label: 'Библиотеки' },
  { slug: 'utility', label: 'Утилиты' },
];

// типы контента каталога (соответствуют project_type на Modrinth)
const MOD_TYPES = {
  mod: { label: 'Моды', action: '+ Установить', folder: 'mods' },
  shader: { label: 'Шейдеры', action: '+ В шейдеры', folder: 'shaders' },
  resourcepack: { label: 'Ресурс-паки', action: '+ В ресурсы', folder: 'resources' },
  modpack: { label: 'Модпаки', action: '⬇ Импорт', folder: 'mods' },
};
// категории имеет смысл только у модов; остальное — просто топ
const CATS_BY_TYPE = {
  mod: MOD_CATS,
  shader: [{ slug: '', label: '★ Топ', desc: 'самые популярные' }],
  resourcepack: [{ slug: '', label: '★ Топ', desc: 'самые популярные' }],
  modpack: [{ slug: '', label: '★ Топ', desc: 'самые популярные' }],
};

let modsState = { type: 'mod', cat: '', page: 1, query: '' };
let searchTimer = null;

// категории
function renderModCats() {
  const cats = CATS_BY_TYPE[modsState.type] || MOD_CATS;
  if (!cats.some((c) => c.slug === modsState.cat)) modsState.cat = '';
  $('#mod-cat-bar').innerHTML = cats.map(
    (c) => `<button class="cat-chip-btn${(modsState.cat === c.slug ? ' active' : '')}" data-cat="${c.slug}" title="${c.desc || ''}">${c.label}</button>`
  ).join('');
  $$('#mod-cat-bar .cat-chip-btn').forEach((b) => {
    b.addEventListener('click', () => {
      modsState.cat = b.dataset.cat;
      modsState.page = 1;
      renderModCats();
      loadMods();
    });
  });
}

// смена типа контента (моды/шейдеры/ресурсы/модпаки)
$('#mod-type').addEventListener('change', () => {
  modsState.type = $('#mod-type').value || 'mod';
  modsState.cat = '';
  modsState.page = 1;
  $('#mod-search').placeholder = 'Поиск: ' + (MOD_TYPES[modsState.type] || MOD_TYPES.mod).label.toLowerCase() + ' на Modrinth...';
  renderModCats();
  loadMods();
});

// Табы модов
$$('#mods-tabs .mtab').forEach((btn) => {
  btn.addEventListener('click', () => {
    $$('#mods-tabs .mtab').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    const tab = btn.dataset.tab;
    // Модпаки: на вкладке инстанс-грид
    $$('.mods-pane').forEach((p) => p.classList.toggle('active', p.id === 'pane-' + tab));
    if (tab === 'instances') renderInstances();
    if (tab === 'installed') renderInstalledMods();
  });
});

$('#mod-search').addEventListener('input', () => {
  $('#mod-search-clear').style.display = $('#mod-search').value ? 'flex' : 'none';
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => { modsState.page = 1; loadMods(); }, 400);
});
$('#mod-search-clear').addEventListener('click', () => {
  $('#mod-search').value = '';
  $('#mod-search-clear').style.display = 'none';
  modsState.page = 1;
  loadMods();
});

function fmtDownloads(n) {
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return String(n);
}

function buildFacets() {
  const type = modsState.type || 'mod';
  const facets = [['project_type:' + type]];
  if (type === 'mod') {
    // моды фильтруем под выбранный инстанс
    facets.push(['versions:' + modMc()], ['categories:' + modLoader()]);
  }
  // шейдеры/ресурсы/модпаки не фильтруем по лоадеру и версии:
  // у шейдеров/ресурсов свои правила совместимости, а модпак сам задаёт MC
  if (modsState.cat) facets.push(['categories:' + modsState.cat]);
  return facets;
}

async function loadMods() {
  const q = $('#mod-search').value.trim();
  modsState.query = q;
  // На первой странице без поиска и без выбранной категории — всегда самый популярный топ
  const query = q || modsState.cat || '';
  const index = modsState.cat || modsState.query
    ? 'relevance'
    : 'downloads'; // в поиске и категориях — по релевантности, в общем топе — по скачиваниям
  const grid = $('#mod-grid');
  let found;
  try {
    found = await window.API.modrinthSearch(query, {
      limit: MOD_PER_PAGE,
      offset: (modsState.page - 1) * MOD_PER_PAGE,
      index,
      facets: buildFacets(),
    });
  } catch (e) {
    found = null;
  }
  if (!Array.isArray(found)) {
    grid.innerHTML = `<div class="mod-card"><h3>Нет связи с Modrinth</h3><p class="mc-desc">Проверь интернет и нажми «›», чтобы повторить.</p></div>`;
    $('#mod-pagination').innerHTML = `
      <button class="pg-btn" data-page="${modsState.page - 1}" ${modsState.page <= 1 ? 'disabled' : ''}>‹</button>
      <span class="pg-info">Страница ${modsState.page}</span>
      <button class="pg-btn" data-page="${modsState.page + 1}">›</button>`;
    $$('#mod-pagination .pg-btn').forEach((b) => {
      b.addEventListener('click', () => { if (!b.disabled) { modsState.page = parseInt(b.dataset.page, 10); loadMods(); } });
    });
    return;
  }
  if (!found.length) {
    grid.innerHTML = `<div class="mod-card"><h3>Ничего не найдено</h3><p class="mc-desc">Попробуй другой запрос, версию или категорию.</p></div>`;
    $('#mod-pagination').innerHTML = '';
    return;
  }
  const installed = installedSlugs();
  const type = modsState.type || 'mod';
  const shFiles = installedFilesSet('shaders');
  const rsFiles = installedFilesSet('resources');
  grid.innerHTML = found
    .map((m) => {
      const isInst = type === 'mod'
        ? installed.has(m.slug)
        : type === 'shader'
          ? fileMatchesSlug(shFiles, m.slug)
          : type === 'resourcepack'
            ? fileMatchesSlug(rsFiles, m.slug)
            : false;
      const icon = m.icon_url
        ? `<img class="mcard-icon" src="${m.icon_url}" alt="" loading="lazy" onerror="this.style.display='none'">`
        : `<div class="mcard-icon placeholder">${(m.title || '?')[0]}</div>`;
      const chip = type !== 'mod' ? `<span class="cat-chip type-chip">${(MOD_TYPES[type] || {}).label || type}</span>` : '';
      return `<div class="mod-card" data-slug="${m.slug}" data-type="${type}">
        <div class="mcard-top">
          ${icon}
          <div class="mcard-info">
            <h3 title="${m.title}">${m.title}</h3>
            <div class="mc-cats">${chip}${(m.categories || []).slice(0, 3).map((c) => `<span class="cat-chip">${c}</span>`).join('')}</div>
          </div>
        </div>
        <p class="mc-desc">${(m.description || '').slice(0, 130)}</p>
        <div class="mcard-meta">
          <span>⬇ ${fmtDownloads(m.downloads || 0)}</span>
          <span>★ ${m.follows ? fmtDownloads(m.follows) : '0'}</span>
        </div>
        <div class="mc-actions">
          <button class="mc-btn ${isInst ? 'installed' : ''}" data-action="install" data-slug="${m.slug}" data-title="${m.title}" data-type="${type}">${isInst ? '✓ Установлен' : (MOD_TYPES[type] || MOD_TYPES.mod).action}</button>
          ${isInst ? `<button class="mc-btn danger" data-action="uninstall" data-slug="${m.slug}" data-title="${m.title}" data-type="${type}">Удалить</button>` : ''}
        </div>
      </div>`;
    })
    .join('');
  renderPagination();
}

function renderPagination() {
  // Приблизительное число страниц (Modrinth не отдаёт total в search v2)
  // Показываем фиксированное число страниц вокруг текущей
  $('#mod-pagination').innerHTML = `
    <button class="pg-btn" data-page="${modsState.page - 1}" ${modsState.page <= 1 ? 'disabled' : ''}>‹</button>
    <span class="pg-info">Страница ${modsState.page}</span>
    <button class="pg-btn" data-page="${modsState.page + 1}">›</button>
  `;
  $$('#mod-pagination .pg-btn').forEach((b) => {
    b.addEventListener('click', () => {
      if (b.disabled) return;
      modsState.page = parseInt(b.dataset.page, 10);
      loadMods();
      $('#mod-grid').scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
  });
}

function installedSlugs() {
  const set = new Set();
  (state.installedMods || []).forEach((m) => {
    if (m.slug) set.add(m.slug.toLowerCase());
    // по имени файла тоже
    const base = m.file.toLowerCase().replace(/-(\d+\.\d+[\w.-]*)+\.jar$/i, '');
    set.add(base);
  });
  return set;
}

// Установленные шейдеры/ресурсы — определяем по имени файла (модпак может не совпасть по slug)
function installedFilesSet(kind) {
  const arr = (state.instFiles || {})[kind] || [];
  const set = new Set();
  arr.forEach((f) => set.add(f.file.toLowerCase()));
  return set;
}
function fileMatchesSlug(files, slug) {
  const s = String(slug || '').toLowerCase();
  for (const f of files) {
    if (f.includes(s) || s.includes(f.replace(/\.(jar|zip)$/i, ''))) return true;
  }
  return false;
}

async function refreshInstalledFiles() {
  if (!state.selectedInstance) { state.instFiles = {}; return; }
  const [sh, rs] = await Promise.all([
    window.API.instanceFiles(state.selectedInstance, 'shaders'),
    window.API.instanceFiles(state.selectedInstance, 'resources'),
  ]);
  state.instFiles = { shaders: sh || [], resources: rs || [] };
}

async function refreshInstalledMods() {
  if (!state.selectedInstance) { state.installedMods = []; state.instFiles = {}; $('#instmod-count').textContent = '0'; return; }
  state.installedMods = await window.API.instanceMods(state.selectedInstance);
  $('#instmod-count').textContent = state.installedMods.length;
  await refreshInstalledFiles();
}

// обработчик кликов по сетке модов (делегирование)
$('#mod-grid').addEventListener('click', async (e) => {
  const btn = e.target.closest('[data-action]');
  const card = e.target.closest('.mod-card');
  if (!card) return;
  const slug = card.dataset.slug;
  const ctype = card.dataset.type || 'mod';

  // клик по кнопке установки/удаления/импорта
  if (btn) {
    const title = btn.dataset.title;

    if (btn.dataset.action === 'install') {
      // импорт модпака создаёт свой инстанс — выбранный не нужен
      if (ctype === 'modpack') {
        btn.disabled = true;
        btn.textContent = '⏳ Импорт...';
        state.importBtn = btn;
        try {
          const res = await window.API.modpackInstall(slug, null);
          if (res && res.ok) {
            flash(`✓ Модпак «${res.name}» (${res.mcVersion} • ${res.loader}): скачано ${res.downloaded}/${res.total}`);
            await renderInstances();
            state.selectedInstance = res.instanceId;
            renderInstances();
            renderModPackBar();
            await refreshInstalledMods();
            // сразу переключаемся на вкладку «Модпаки», чтобы видеть результат
            const tabBtn = document.querySelector('#mods-tabs .mtab[data-tab="instances"]');
            if (tabBtn) tabBtn.click();
          } else {
            flash('✗ ' + ((res && res.error) || 'ошибка импорта'));
          }
        } catch (err) {
          flash('✗ Не удалось импортировать ' + title);
        }
        // в любом случае возвращаем кнопку — иначе она навсегда останется
        // «⏳ 100%» disabled, и повторный импорт будет невозможен
        state.importBtn = null;
        btn.textContent = (MOD_TYPES.modpack).action;
        btn.disabled = false;
        return;
      }

      if (!state.selectedInstance) { flash('Сначала создай/выбери инстанс'); return; }
      btn.disabled = true;
      btn.textContent = '⏳ ...';
      try {
        const loader = modLoader();
        const mc = modMc();
        const res = await window.API.modInstall({ slug, mcVersion: mc, loader, instanceId: state.selectedInstance, type: ctype });
        if (res.ok) {
          if (ctype === 'mod') await window.API.modDeps({ slug, mcVersion: mc, loader, instanceId: state.selectedInstance });
          const where = ctype === 'shader' ? 'в шейдеры' : ctype === 'resourcepack' ? 'в ресурс-паки' : '';
          flash(`✓ ${title} установлено ${where} (${mc})`);
        } else {
          flash('✗ ' + (res.error || 'ошибка'));
        }
      } catch (err) {
        flash('✗ Не удалось установить ' + title);
      }
      await refreshInstalledMods();
      await loadMods();
      renderInstances();
      return;
    }

    if (btn.dataset.action === 'uninstall') {
      if (ctype === 'mod') {
        const done = await window.API.instanceModDelete(state.selectedInstance, slug);
        if (done) flash('Мод удалён');
      } else if (ctype === 'shader' || ctype === 'resourcepack') {
        if (!state.selectedInstance) { flash('Сначала выбери инстанс'); return; }
        const kind = ctype === 'shader' ? 'shaders' : 'resources';
        const files = (state.instFiles || {})[kind] || [];
        const hit = files.filter((f) => fileMatchesSlug([f.file.toLowerCase()], slug));
        if (!hit.length) { flash('Файл не найден в этом инстансе'); return; }
        let n = 0;
        for (const f of hit) if (await window.API.instanceFileDelete(state.selectedInstance, kind, f.file)) n++;
        flash(n ? `Удалено из ${ctype === 'shader' ? 'шейдеров' : 'ресурсов'}: ${n}` : 'Не удалось удалить');
      }
      await refreshInstalledMods();
      await loadMods();
      renderInstances();
      return;
    }
  }

  // клик по самой карточке — открыть страницу мода как на Modrinth
  openModDetail(slug, ctype);
});

// ===== Mod detail page (как на Modrinth) =====
// Версии MC для селекта: активная ВСЕГДА первой, иначе браузер молча
// выберет свежий снапшот (под него версий нет). Без обрезки: сначала все
// release (свежие сверху), потом snapshot — как в официальном лаунчере.
function mcVersionOptions() {
  const cur = modMc();
  const all = state.mcVersions || [];
  const typeOf = new Map(all.map((v) => [v.id, v.type]));
  const releases = all.filter((v) => v.type === 'release' || !v.type).map((v) => v.id);
  const old = all.filter((v) => v.type === 'old_release' || v.type === 'old_alpha' || v.type === 'old_beta').map((v) => v.id);
  const snaps = all.filter((v) => v.type === 'snapshot').map((v) => v.id);
  const ordered = [cur, ...releases.filter((v) => v !== cur), ...snaps.filter((v) => v !== cur), ...old.filter((v) => v !== cur)].filter(Boolean);
  const oldTypes = new Set(['old_release', 'old_alpha', 'old_beta']);
  return ordered
    .map((v) => {
      const t = typeOf.get(v);
      const mark = v === cur ? ' • активный'
        : t === 'snapshot' ? ' (снапшот)'
        : oldTypes.has(t) ? ' (старая)'
        : '';
      return `<option value="${v}" ${v === cur ? 'selected' : ''}>${v}${mark}</option>`;
    })
    .join('');
}

async function openModDetail(slug, ctype) {
  const type = ctype || 'mod';
  const proj = await window.API.modrinthProject(slug);
  if (!proj) { flash('Не удалось загрузить страницу'); return; }

  const wrap = document.createElement('div');
  wrap.className = 'mod-overlay';
  const CLOSE = 'position:absolute;top:14px;right:16px;background:transparent;border:1px solid var(--stroke);color:var(--dim);width:34px;height:34px;border-radius:9px;cursor:pointer;font-size:1.1rem';
  wrap.innerHTML = `
    <div class="mod-overlay-card">
      <button onclick="this.closest('.mod-overlay').remove()" style="${CLOSE}">✕</button>

      <div class="md-head">
        ${proj.icon_url
          ? `<img class="md-icon" src="${proj.icon_url}" alt="" onerror="this.style.display='none'">`
          : `<div class="md-icon placeholder">${(proj.title || '?')[0]}</div>`}
        <div class="md-headinfo">
          <div class="md-title">${(proj.title || '').replace(/</g, '&lt;')}</div>
          <div class="md-author">by ${(proj.author || 'unknown').replace(/</g, '&lt;')}${proj.team ? ' и др.' : ''}</div>
          <div class="md-chips">
            ${(proj.categories || []).slice(0, 6).map((c) => `<span class="cat-chip">${c}</span>`).join('')}
            ${(proj.loaders || []).slice(0, 4).map((l) => `<span class="cat-chip loader-chipmal">${l}</span>`).join('')}
          </div>
        </div>
        <div class="md-stats">
          <div class="md-stat"><b>${fmtDownloads(proj.downloads || 0)}</b><span>скачиваний</span></div>
          <div class="md-stat"><b>${fmtDownloads(proj.follows || 0)}</b><span>следят</span></div>
          <div class="md-stat"><b>${(proj.version_type || '-')}</b><span>тип</span></div>
        </div>
      </div>

      <div class="md-tabs">
        <button class="mdtab active" data-mtab="versions">Версии</button>
        <button class="mdtab" data-mtab="desc">Описание</button>
        <button class="mdtab" data-mtab="deps">Зависимости</button>
      </div>

      <div class="md-body">
        <div class="md-pane active" data-mpane="versions">
          <div class="md-version-controls" ${type !== 'mod' ? 'data-noloader="1"' : ''}>
            <label>${type === 'modpack' ? 'Версия сборки' : 'Активный модпак'}</label>
            <span class="md-packinfo">${type === 'modpack'
              ? 'Импорт создаст новый модпак со всеми модами'
              : (activeInstance() ? (activeInstance().name + ' — ' + activeInstance().mcVersion + ' • ' + activeInstance().loader) : 'Не выбран')}</span>
            <label>Версия MC</label>
            <select id="md-mcver">${mcVersionOptions()}</select>
            <label class="md-loaderlabel">Ядро</label>
            <select id="md-loader" class="md-loadersel">
              <option value="fabric" ${modLoader()==='fabric'?'selected':''}>Fabric</option>
              <option value="forge" ${modLoader()==='forge'?'selected':''}>Forge</option>
              <option value="neoforge" ${modLoader()==='neoforge'?'selected':''}>NeoForge</option>
            </select>
            <button class="btn-ghost" id="md-reload">Показать версии</button>
          </div>
          <div class="md-version-list" id="md-version-list">Загрузка...</div>
        </div>
        <div class="md-pane" data-mpane="desc">
          <div class="md-bodytext">
            <h3>О моде</h3>
            <div>${(proj.body || proj.description || 'Нет описания').replace(/</g, '&lt;').slice(0, 3000) || 'Нет описания'}</div>
            <div class="md-projlinks">${(proj.issues_url || proj.source_url || proj.support_url) ? '<h3 style="margin-top:1rem">Ссылки</h3>' : ''}
              ${proj.issues_url ? `<a href="#" onclick="window.API.openExternal('${proj.issues_url}');return false">Issues</a>` : ''}
              ${proj.source_url ? ` <a href="#" onclick="window.API.openExternal('${proj.source_url}');return false">Source</a>` : ''}</div>
          </div>
        </div>
        <div class="md-pane" data-mpane="deps">
          <div class="md-bodytext" id="md-deps">Вкладка «Версии» — нажми на версию, чтобы увидеть зависимости.</div>
        </div>
      </div>
    </div>`;
  document.body.appendChild(wrap);
  wrap.dataset.ptype = type;
  // у шейдеров/ресурсов/модпаков нет лоадера — прячем выбор ядра
  if (type !== 'mod') {
    const lsel = $('#md-loader', wrap);
    const llab = $('.md-loaderlabel', wrap);
    if (lsel) lsel.style.display = 'none';
    if (llab) llab.style.display = 'none';
  }
  wrap.addEventListener('click', (e) => { if (e.target === wrap) wrap.remove(); });

  // табы
  $$('#md-tabs .mdtab', wrap).forEach((t) => t.addEventListener('click', () => {
    $$('.mdtab', wrap).forEach((x) => x.classList.remove('active'));
    t.classList.add('active');
    const tab = t.dataset.mtab;
    $$('.md-pane', wrap).forEach((p) => p.classList.toggle('active', p.dataset.mpane === tab));
    if (tab === 'deps') renderDeps(wrap, null);
  }));

  $('#md-reload', wrap).addEventListener('click', () => renderModVersions(wrap, proj));
  $('#md-mcver', wrap).addEventListener('change', () => renderModVersions(wrap, proj));
  $('#md-loader', wrap).addEventListener('change', () => renderModVersions(wrap, proj));
  renderModVersions(wrap, proj);
}

async function renderModVersions(wrap, proj) {
  const type = wrap.dataset.ptype || 'mod';
  const mc = $('#md-mcver', wrap).value;
  const loader = $('#md-loader', wrap).value;
  const list = $('#md-version-list', wrap);
  list.innerHTML = '<div class="md-hint">Загрузка версий...</div>';
  const versions = await window.API.modrinthVersions(proj.slug, mc, loader, type);
  if (!versions || !versions.length) {
    list.innerHTML = `<div class="md-hint">Нет версий ${proj.title} для Minecraft ${mc}${type === 'mod' ? ' на ' + loader : ''}.</div>`;
    return;
  }
  const btnLabel = type === 'modpack' ? 'Импорт' : type === 'shader' ? 'В шейдеры' : type === 'resourcepack' ? 'В ресурсы' : 'Установить';
  list.innerHTML = versions.map((v, i) => {
    const primary = v.files && v.files[0];
    return `<div class="md-version-row" data-vname="${v.name && v.name.replace(/"/g, '&quot;')}" data-file="${primary ? primary.url : ''}" data-filename="${primary ? primary.filename : ''}">
      <div class="md-ver-main">
        <span class="md-ver-name">${(v.name || 'v' + v.version_number).replace(/</g, '&lt;')}</span>
        <span class="md-ver-type ${(v.version_type || 'release').toLowerCase()}">${v.version_type || 'release'}</span>
      </div>
      <div class="md-ver-meta">
        <span>${v.date_published ? v.date_published.slice(0, 10) : ''}</span>
        <span>${(v.game_versions || []).slice(0, 3).join(', ')}</span>
        <button class="mc-btn" data-vinstall="1">${btnLabel}</button>
      </div>
    </div>`;
  }).join('');

  $$('.md-version-row', wrap).forEach((row) => {
    row.addEventListener('click', (e) => {
      e.stopPropagation();
      const act = e.target.closest('[data-vinstall]');
      if (act) { installModVersion(wrap, proj, row.dataset.file, row.dataset.filename, row.dataset.vname, act); return; }
      // показать зависимости версии
      renderDeps(wrap, versions.find((v) => v.name === row.dataset.vname));
      $$('.md-version-row', wrap).forEach((r) => r.classList.remove('selected'));
      row.classList.add('selected');
    });
  });
}

async function installModVersion(wrap, proj, fileUrl, filename, vname, btn) {
  const type = wrap.dataset.ptype || 'mod';
  const mc = $('#md-mcver', wrap).value;
  const loader = $('#md-loader', wrap).value;
  btn.disabled = true;
  btn.textContent = '⏳...';

  // модпак из карточки деталей — полный импорт (создаёт свой инстанс)
  if (type === 'modpack') {
    state.importBtn = btn;
    const res = await window.API.modpackInstall(proj.slug, mc);
    if (res && res.ok) {
      flash(`✓ Модпак «${res.name}» импортирован (${res.downloaded}/${res.total})`);
      await renderInstances();
      state.selectedInstance = res.instanceId;
      renderInstances();
      renderModPackBar();
      await refreshInstalledMods();
      const tabBtn = document.querySelector('#mods-tabs .mtab[data-tab="instances"]');
      if (tabBtn) tabBtn.click();
      const ov = wrap.closest('.mod-overlay'); if (ov) ov.remove();
    } else {
      flash('✗ ' + ((res && res.error) || 'ошибка импорта'));
    }
    state.importBtn = null;
    btn.textContent = 'Импорт';
    btn.disabled = false;
    return;
  }

  if (!state.selectedInstance) { flash('Сначала выбери модпак'); btn.textContent = 'Установить'; btn.disabled = false; return; }
  const res = await window.API.modInstallDirect({ instanceId: state.selectedInstance, url: fileUrl, filename, name: vname, slug: proj.slug, type });
  if (res && res.ok) {
    if (type === 'mod') await window.API.modDeps({ slug: proj.slug, mcVersion: mc, loader, instanceId: state.selectedInstance });
    flash('✓ Установлено: ' + vname);
    btn.textContent = '✓';
  } else {
    flash('✗ ' + ((res && res.error) || 'ошибка'));
    btn.textContent = 'Установить';
    btn.disabled = false;
  }
  await refreshInstalledMods();
  renderInstances();
}

function renderDeps(wrap, version) {
  const box = $('#md-deps', wrap);
  if (!version || !version.dependencies || !version.dependencies.length) {
    box.innerHTML = '<div class="md-hint">Зависимостей нет.</div>';
    return;
  }
  const required = version.dependencies.filter((d) => d.dependency_type === 'required');
  const other = version.dependencies.filter((d) => d.dependency_type !== 'required');
  box.innerHTML = '<div class="md-hint">' + version.name + '</div>';
  if (required.length) {
    box.innerHTML += '<h4 style="margin:0.7rem 0 0.4rem;color:var(--mint)">Обязательные</h4>' +
      required.map((d) => `<div class="dep-row">${d.project_id ? 'Зависимый проект ID: ' + d.project_id : (d.version_id || 'версия')} <span class="cat-chip">${d.dependency_type}</span> ${d.file_name || ''}</div>`).join('');
  }
  if (other.length) {
    box.innerHTML += '<h4 style="margin:0.7rem 0 0.4rem;color:var(--dim)">Необязательные</h4>' +
      other.map((d) => `<div class="dep-row">${d.project_id ? 'Проект: ' + d.project_id : (d.version_id || '')} <span class="cat-chip">${d.dependency_type}</span> ${d.file_name || ''}</div>`).join('');
  }
}

// ===== Installed pane: моды + шейдеры + ресурсы выбранного инстанса =====
async function renderInstalledMods() {
  const list = $('#installed-list');
  if (!state.selectedInstance) {
    list.innerHTML = `<div class="mod-card"><p class="mc-desc">Нет выбранного инстанса.</p></div>`;
    return;
  }
  const mods = await window.API.instanceMods(state.selectedInstance);
  await refreshInstalledFiles();
  const instFiles = state.instFiles || {};
  const shaders = instFiles.shaders || [];
  const resources = instFiles.resources || [];

  const section = (title, hint, rows) => `
    <div class="inst-group">
      <h4 class="inst-group-title">${title}<span class="inst-group-count">${rows.length}</span></h4>
      ${rows.length ? rows.join('') : `<p class="inst-empty">${hint}</p>`}
    </div>`;

  const row = (file, size, kind) => `
    <div class="inst-row" data-file="${file}" data-kind="${kind}">
      <span class="inst-name">${file}</span>
      <span class="inst-size">${size ? (size / 1024).toFixed(0) + ' KB' : ''}</span>
      <button class="mc-btn danger" data-ufile="${file}" data-ukind="${kind}">Удалить</button>
    </div>`;

  const modRows = mods.map((m) => row(m.file, m.size, 'mods'));
  const shaderRows = shaders.map((s) => row(s.file, s.size, 'shaders'));
  const resRows = resources.map((r) => row(r.file, r.size, 'resources'));

  if (!mods.length && !shaderRows.length && !resRows.length) {
    list.innerHTML = `<div class="mod-card"><p class="mc-desc">Пусто. Иди в «Каталог» и добавь — для шейдеров/ресурсов выбери тип контента сверху.</p></div>`;
    return;
  }
  list.innerHTML =
    section('Моды', 'Модов нет.', modRows) +
    section('Шейдеры', 'Шейдеров нет — выбери «Шейдеры» в каталоге.', shaderRows) +
    section('Ресурс-паки', 'Ресурс-паков нет — выбери «Ресурс-паки» в каталоге.', resRows);

  $$('.inst-row [data-ufile]').forEach((b) => {
    b.addEventListener('click', async () => {
      const kind = b.dataset.ukind;
      if (kind === 'mods') {
        await window.API.instanceModDeleteFile(state.selectedInstance, b.dataset.ufile);
      } else {
        const done = await window.API.instanceFileDelete(state.selectedInstance, kind, b.dataset.ufile);
        if (!done) { flash('Не удалось удалить файл'); return; }
      }
      flash('Удалено: ' + b.dataset.ufile);
      await renderInstalledMods();
      await refreshInstalledMods();
      renderInstances();
      loadMods();
    });
  });
}

// ===== Тема: свой фон + авто-подсветка из фона =====
const DEFAULT_ACCENT = '#f0a428';
function rgbToHsl(r, g, b) {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  let h = 0, s = 0, l = (max + min) / 2;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r: h = (g - b) / d + (g < b ? 6 : 0); break;
      case g: h = (b - r) / d + 2; break;
      default: h = (r - g) / d + 4;
    }
    h *= 60;
  }
  return [h, s, l];
}
function hslToHex(h, s, l) {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs((h / 60) % 2 - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; } else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; } else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; } else { r = c; b = x; }
  const to = (v) => Math.round((v + m) * 255).toString(16).padStart(2, '0');
  return '#' + to(r) + to(g) + to(b);
}
function rgbaStr(hex, a) {
  const r = parseInt(hex.slice(1, 3), 16), g = parseInt(hex.slice(3, 5), 16), b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${a})`;
}
function shiftHue(hex, deg, sMul, lMul) {
  const r = parseInt(hex.slice(1, 3), 16), g = parseInt(hex.slice(3, 5), 16), b = parseInt(hex.slice(5, 7), 16);
  const [h, s, l] = rgbToHsl(r, g, b);
  return hslToHex((h + deg) % 360, Math.min(1, s * (sMul || 1)), Math.min(1, l * (lMul || 1)));
}
function extractPalette(dataUrl) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      try {
        const size = 64;
        const c = document.createElement('canvas');
        c.width = size; c.height = size;
        const ctx = c.getContext('2d', { willReadFrequently: true });
        ctx.drawImage(img, 0, 0, size, size);
        const d = ctx.getImageData(0, 0, size, size).data;
        const buckets = new Map();
        for (let i = 0; i < d.length; i += 4) {
          if (d[i + 3] < 120) continue;
          const rr = d[i], gg = d[i + 1], bb = d[i + 2];
          const [h, s, l] = rgbToHsl(rr, gg, bb);
          if (l < 0.08 || l > 0.92) continue; // фон/шум: почти чёрный/белый
          const key = (rr >> 4) + ',' + (gg >> 4) + ',' + (bb >> 4);
          const el = buckets.get(key) || { rr, gg, bb, count: 0, ss: 0, sl: 0 };
          el.rr = rr; el.gg = gg; el.bb = bb; el.count++; el.ss += s; el.sl += l;
          buckets.set(key, el);
        }
        const clusters = [];
        buckets.forEach((el) => {
          const avgS = el.ss / el.count;
          const avgL = el.sl / el.count;
          // «тон + количество + цвет»: вес по количеству пикселей, насыщенности и яркости
          const score = Math.pow(el.count, 0.85) * (0.5 + avgS) * (0.85 + avgL * 0.35);
          const [h] = rgbToHsl(el.rr, el.gg, el.bb);
          clusters.push({
            h,
            count: el.count,
            score,
            hex: hslToHex(h, Math.min(1, avgS * 1.2), Math.min(0.72, Math.max(0.5, avgL))),
          });
        });
        clusters.sort((a, b) => b.score - a.score);
        resolve(clusters.slice(0, 5));
      } catch (e) { resolve([]); }
    };
    img.onerror = () => resolve([]);
    img.src = dataUrl;
  });
}
function paletteFromTop(top) {
  const ac = (top[0] && top[0].hex) || DEFAULT_ACCENT;
  // Второй акцент: выбираем кластер с заметно другим тоном (преобладает по количеству),
  // иначе — сдвиг тона от первого.
  const second = top.find((cl) => {
    const diff = Math.abs(cl.h - (top[0] ? top[0].h : 0));
    return diff > 28 && diff < 332;
  });
  return { ac, ac2: second ? second.hex : shiftHue(ac, 38, 1.1, 0.95) };
}
function applyBg(dataUrl) {
  if (!dataUrl) {
    document.documentElement.style.removeProperty('--user-bg');
    document.body.classList.remove('has-bg');
    const prev = $('#bg-preview');
    if (prev) { prev.style.backgroundImage = ''; prev.classList.remove('filled'); }
    return;
  }
  document.documentElement.style.setProperty('--user-bg', 'url("' + dataUrl + '")');
  document.body.classList.add('has-bg');
  const prev = $('#bg-preview');
  if (prev) { prev.style.backgroundImage = 'url("' + dataUrl + '")'; prev.classList.add('filled'); }
}
function applyAccent(hex, hex2) {
  if (!hex) return;
  const root = document.documentElement.style;
  const ac2 = hex2 || shiftHue(hex, 38, 1.1, 0.95);
  root.setProperty('--ac', hex);
  root.setProperty('--ac2', ac2);
  root.setProperty('--acsoft', rgbaStr(hex, 0.14));
  root.setProperty('--acsoft2', rgbaStr(ac2, 0.08));
  root.setProperty('--glow1', rgbaStr(hex, 0.15));
  root.setProperty('--glow2', rgbaStr(ac2, 0.12));
  root.setProperty('--sh', rgbaStr(hex, 0.4));
  const picker = $('#set-accent');
  if (picker) picker.value = hex;
}
function currentAccentHex() {
  const root = getComputedStyle(document.documentElement);
  const v = root.getPropertyValue('--ac').trim();
  return v.startsWith('#') ? v : '#4f8cff';
}
async function themeFromBg(dataUrl) {
  if (!dataUrl) return { ac: DEFAULT_ACCENT, ac2: shiftHue(DEFAULT_ACCENT, 38, 1.1, 0.95) };
  const pal = await extractPalette(dataUrl);
  return paletteFromTop(pal);
}
async function initTheme() {
  const saved = await window.API.getBg();
  applyBg(saved && saved.dataUrl ? saved.dataUrl : null);
  const t = await themeFromBg(saved && saved.dataUrl ? saved.dataUrl : null);
  applyAccent(t.ac, t.ac2);
  $('#set-accent').value = currentAccentHex();
  buildStylePresets(); // карточки базового стиля — до applyCust, чтобы он подсветил активную
}
$('#btn-bg-browse').addEventListener('click', async () => {
  const p = await window.API.pickBg();
  if (!p) return;
  const res = await window.API.setBg(p);
  if (res && res.ok) {
    $('#set-bg-path').value = res.path;
    applyBg(res.dataUrl);
    const t = await themeFromBg(res.dataUrl);
    applyAccent(t.ac, t.ac2);
    flash('Фон обновлён: подсветка из доминирующих цветов фона');
  } else flash('Не удалось применить фон');
});
$('#btn-bg-clear').addEventListener('click', async () => {
  await window.API.clearBg();
  $('#set-bg-path').value = '';
  applyBg(null);
  applyAccent(DEFAULT_ACCENT);
  $('#set-accent').value = DEFAULT_ACCENT;
  flash('Фон сброшен (стандартный)');
});
$('#set-accent').addEventListener('input', () => applyAccent($('#set-accent').value));
$('#btn-accent-reset').addEventListener('click', async () => {
  const saved = await window.API.getBg();
  const t = await themeFromBg(saved && saved.dataUrl ? saved.dataUrl : null);
  $('#set-accent').value = t.ac;
  applyAccent(t.ac, t.ac2);
  flash('Акцент: авто из доминирующих цветов фона');
});

// ===== Статистика сессий (playtime) =====
async function loadPlaytime(full) {
  playtime = (await window.API.statsGet()) || {};
  renderTelemetry();
  if (full !== false) { renderInstances(); renderPlay(); }
}
window.API.onStatsChanged(() => loadPlaytime(false));

// ===== Линк: хаб лаунчеров =====
const LINKS_KEY = 'fr_links';
let launchers = [];
let dragIdx = null;
try { launchers = JSON.parse(localStorage.getItem(LINKS_KEY) || '[]'); } catch (e) {}
function saveLaunchers() { localStorage.setItem(LINKS_KEY, JSON.stringify(launchers)); }
// ----- Запущенные лаунчеры: отслеживание + статистика -----
let launcherTabs = [];   // [{key, name, exe, startedAt}] из main
let linkStats = {};      // exeLower -> { name, sessions, totalMs }

function linkNormPath(p) { return String(p || '').toLowerCase(); }

// все запущенные сессии для данного .exe
function runningFor(path) {
  const k = linkNormPath(path);
  return launcherTabs.filter((t) => linkNormPath(t.exe) === k);
}

function fmtDur(ms) {
  ms = Math.max(0, ms | 0);
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const p2 = (n) => String(n).padStart(2, '0');
  return h + ':' + p2(m) + ':' + p2(s);
}

function linkStatFor(path) {
  return linkStats[linkNormPath(path)] || { sessions: 0, totalMs: 0 };
}

// накопленное время + текущие живые сессии
function liveMs(path) {
  const base = linkStatFor(path).totalMs || 0;
  const now = Date.now();
  let extra = 0;
  runningFor(path).forEach((t) => { extra += Math.max(0, now - (t.startedAt || now)); });
  return base + extra;
}

window.API.onEmbedsChanged(async (d) => {
  launcherTabs = (d && d.list) || [];
  try { linkStats = await window.API.linkStats(); } catch (e) {}
  const page = document.querySelector('.page.active');
  if (page && page.id === 'page-link') renderLaunchers();
});

function renderLaunchers() {
  const grid = $('#link-grid');
  if (!grid) return;
  if (!launchers.length) {
    grid.innerHTML = `<div class="friend-empty">Лаунчеров нет. Нажми «+ Добавить лаунчер» и выбери .exe.</div>`;
    return;
  }
  grid.innerHTML = launchers.map((l, i) => {
    const running = runningFor(l.path);
    const isRun = running.length > 0;
    const st = linkStatFor(l.path);
    return `<div class="link-card ${isRun ? 'running' : ''}" draggable="true" data-lidx="${i}">
    <button class="link-del" data-ldel="${i}" title="Удалить">✕</button>
    <div class="link-ico-wrap">${l.icon ? `<img class="link-ico" src="${l.icon}" alt="">` : `<div class="link-ico ph">${(l.name[0] || '⇪').toUpperCase()}</div>`}</div>
    <div class="link-name">${l.name.replace(/</g, '&lt;')}</div>
    <div class="link-status ${isRun ? 'on' : ''}">${isRun ? '● Запущен' : ''}</div>
    <div class="link-stats">
      <span title="Количество запусков">▶ <b data-lsess="${i}">${st.sessions || 0}</b></span>
      <span title="Общее время">⏱ <b data-ltime="${i}">${fmtDur(liveMs(l.path))}</b></span>
    </div>
    <div class="link-path" title="${l.path}">${l.path.replace(/</g, '&lt;')}</div>
    <div class="link-actions">
      <button class="btn-primary link-run" data-lrun="${i}">Запустить</button>
      ${isRun ? `<button class="btn-ghost link-stop" data-lstop="${i}">Закрыть</button>` : ''}
    </div>
  </div>`;
  }).join('');

  $$('.link-card').forEach((card) => {
    card.addEventListener('dragstart', () => { dragIdx = parseInt(card.dataset.lidx, 10); card.classList.add('dragging'); });
    card.addEventListener('dragend', () => { card.classList.remove('dragging'); dragIdx = null; });
    card.addEventListener('dragover', (e) => e.preventDefault());
    card.addEventListener('drop', (e) => {
      e.preventDefault();
      const to = parseInt(card.dataset.lidx, 10);
      if (dragIdx === null || dragIdx === to) return;
      const [m] = launchers.splice(dragIdx, 1);
      launchers.splice(to, 0, m);
      saveLaunchers();
      renderLaunchers();
      dragIdx = null;
    });
  });
  $$('[data-ldel]').forEach((b) => b.addEventListener('click', (e) => {
    e.stopPropagation();
    launchers.splice(parseInt(b.dataset.ldel, 10), 1);
    saveLaunchers();
    renderLaunchers();
  }));
  $$('[data-lrun]').forEach((b) => b.addEventListener('click', async (e) => {
    e.stopPropagation();
    const l = launchers[parseInt(b.dataset.lrun, 10)];
    if (!l) return;
    const before = b.innerHTML;
    b.disabled = true;
    b.textContent = 'Запускаю...';
    const res = await window.API.linkTabLaunch(l.path);
    b.disabled = false;
    if (res && res.ok) {
      try { linkStats = await window.API.linkStats(); } catch (err) {}
      flash(l.name + ' запущен отдельным окном');
      renderLaunchers();
    } else {
      b.innerHTML = before;
      flash('Ошибка: ' + ((res && res.error) || 'не удалось запустить'));
    }
  }));
  $$('[data-lstop]').forEach((b) => b.addEventListener('click', async (e) => {
    e.stopPropagation();
    const l = launchers[parseInt(b.dataset.lstop, 10)];
    if (!l) return;
    for (const t of runningFor(l.path)) await window.API.linkTabClose(t.key);
    try { linkStats = await window.API.linkStats(); } catch (err) {}
    renderLaunchers();
  }));
}
$('#btn-link-add').addEventListener('click', async () => {
  const exe = await window.API.linkPick();
  if (!exe) return;
  const name = exe.split('\\').pop().split('/').pop().replace(/\.exe$/i, '').replace(/[_-]+/g, ' ').trim() || 'Лаунчер';
  const icon = await window.API.linkExtractIcon(exe);
  launchers.push({ path: exe, name, icon });
  saveLaunchers();
  renderLaunchers();
  flash('Лаунчер добавлен: ' + name);
});

// живой таймер времени у карточек раздела «Линк»
setInterval(() => {
  const page = document.querySelector('#page-link');
  if (!page || !page.classList.contains('active')) return;
  $$('[data-ltime]').forEach((el) => {
    const l = launchers[parseInt(el.dataset.ltime, 10)];
    if (l) el.textContent = fmtDur(liveMs(l.path));
  });
}, 1000);

// ===== Customization: nav position / theme / density / radius / glow =====
const CUST_KEY = 'fr_cust';
const CUST_DEFAULTS = { nav: 'top', theme: 'void', density: 'cozy', radius: 'soft', glow: 'on', style: 'void' };
state.cust = { ...CUST_DEFAULTS };
try { Object.assign(state.cust, JSON.parse(localStorage.getItem(CUST_KEY) || '{}')); } catch (e) {}

// Акцент каждой темы (используется, когда своего фона нет или тему выбрали вручную)
const THEME_ACCENTS = {
  void:    ['#f0a428', '#ff7a3c'],
  ember:   ['#ff5a2c', '#ff9a4a'],
  toxic:   ['#8fd94a', '#c8f06a'],
  crimson: ['#f0585f', '#ff8a6a'],
  ice:     ['#4fd1e0', '#6ea8ff'],
  mono:    ['#c9ced6', '#8a919b'],
};

function segSet(sel, val) {
  $$(sel + ' .seg-btn').forEach((b) => b.classList.toggle('active', b.dataset.value === val));
}

// ===== Базовый стиль: 5 пресетов =====
// Каждый пресет — готовый образ целиком: фон+акцент (тема), шрифт (data-style),
// плотность, углы, свечение и положение навигации. Всё это остаётся обычными
// настройками — после пресета любой контрол можно докрутить под себя.
const STYLE_PRESETS = [
  { id: 'void',    label: 'Классика', desc: 'Привычный вид: янтарь, мягкие углы',            theme: 'void',    density: 'cozy',    radius: 'soft',  glow: 'on',  nav: 'top' },
  { id: 'ember',   label: 'Жар',      desc: 'Огненный акцент, острые углы, плотно',          theme: 'ember',   density: 'compact', radius: 'sharp', glow: 'on',  nav: 'top' },
  { id: 'toxic',   label: 'Токсик',   desc: 'Кислотная зелень и моноширинный шрифт',         theme: 'toxic',   density: 'compact', radius: 'round', glow: 'on',  nav: 'left' },
  { id: 'crimson', label: 'Багрянец', desc: 'Тёмно-красный, просторный, драматичный',        theme: 'crimson', density: 'roomy',   radius: 'soft',  glow: 'on',  nav: 'top' },
  { id: 'ice',     label: 'Лёд',      desc: 'Холодный голубой, чисто и моно-заголовки',      theme: 'ice',     density: 'roomy',   radius: 'round', glow: 'off', nav: 'top' },
];

function applyStylePreset(id) {
  const p = STYLE_PRESETS.find((s) => s.id === id);
  if (!p) return;
  Object.assign(state.cust, {
    style: p.id, theme: p.theme, density: p.density,
    radius: p.radius, glow: p.glow, nav: p.nav,
  });
  applyCust(true); // пресет всегда меняет акцент — применяем принудительно
  if (typeof flash === 'function') flash('Стиль: ' + p.label);
}

function buildStylePresets() {
  const wrap = $('#style-presets');
  if (!wrap || wrap.dataset.built) return;
  wrap.dataset.built = '1';
  wrap.innerHTML = STYLE_PRESETS.map((p) => {
    const a = THEME_ACCENTS[p.theme] || THEME_ACCENTS.void;
    return `<button type="button" class="style-card" data-style="${p.id}" title="${p.desc}">
      <span class="sc-dots"><i class="sc-dot" style="background:${a[0]}"></i><i class="sc-dot" style="background:${a[1]}"></i></span>
      <span class="sc-name">${p.label}</span>
      <span class="sc-desc">${p.desc}</span>
    </button>`;
  }).join('');
  $$('#style-presets .style-card').forEach((b) => b.addEventListener('click', () => applyStylePreset(b.dataset.style)));
}

function applyCust(forceAccent) {
  const c = state.cust;
  document.body.dataset.nav = c.nav;
  document.body.dataset.theme = c.theme;
  document.body.dataset.density = c.density;
  document.body.dataset.radius = c.radius;
  document.body.dataset.glow = c.glow;
  document.body.dataset.style = c.style || 'void';

  segSet('#cust-nav', c.nav);
  segSet('#cust-density', c.density);
  segSet('#cust-radius', c.radius);
  $$('#cust-theme .theme-chip').forEach((b) => b.classList.toggle('active', b.dataset.theme === c.theme));
  $$('#style-presets .style-card').forEach((b) => b.classList.toggle('active', b.dataset.style === (c.style || 'void')));

  const tg = $('#cust-glow');
  if (tg) {
    const on = c.glow === 'on';
    tg.classList.toggle('on', on);
    tg.setAttribute('aria-checked', on ? 'true' : 'false');
  }

  // Акцент: у своего фона приоритет; иначе акцент задаёт тема (или принудительно при смене темы)
  const hasBg = document.body.classList.contains('has-bg');
  if ((forceAccent || !hasBg) && typeof applyAccent === 'function') {
    const a = THEME_ACCENTS[c.theme] || THEME_ACCENTS.void;
    applyAccent(a[0], a[1]);
  }

  localStorage.setItem(CUST_KEY, JSON.stringify(c));
}

function bindSeg(sel, key) {
  $$(sel + ' .seg-btn').forEach((b) => b.addEventListener('click', () => {
    state.cust[key] = b.dataset.value;
    applyCust(true);
  }));
}
bindSeg('#cust-nav', 'nav');
bindSeg('#cust-density', 'density');
bindSeg('#cust-radius', 'radius');

$$('#cust-theme .theme-chip').forEach((b) => b.addEventListener('click', () => {
  state.cust.theme = b.dataset.theme;
  applyCust(true);
  if (typeof flash === 'function') flash('Тема: ' + b.textContent.trim());
}));

const custGlowBtn = $('#cust-glow');
if (custGlowBtn) custGlowBtn.addEventListener('click', () => {
  state.cust.glow = state.cust.glow === 'on' ? 'off' : 'on';
  applyCust(false);
});

const custResetBtn = $('#btn-cust-reset');
if (custResetBtn) custResetBtn.addEventListener('click', () => {
  state.cust = { ...CUST_DEFAULTS };
  applyCust(true);
  if (typeof flash === 'function') flash('Оформление сброшено');
});

// ===== Init =====
(async () => {
  await initJava();
  await initMcVersions();
  await renderInstances();
  renderTelemetry();
  renderAccount();
  await refreshInstalledMods();
  renderModCats();
  loadMods();
  await initTheme();
  applyCust(false);
  await loadPlaytime();
  const emb = await window.API.linkTabList();
  launcherTabs = (emb && emb.list) || [];
  try { linkStats = await window.API.linkStats(); } catch (e) { linkStats = {}; }
  renderLaunchers();
})();

// ===== Прогрев вкладок =====
// Первое открытие вкладок раньше подтормаживало: браузер строил layout и
// стили скрытых страниц прямо в кадр анимации. В простое после загрузки
// синхронно проходим по всем страницам (промежуточное состояние не рисуется —
// кадр не выводится, пока не отпустим стек) и повторяем тяжёлые рендеры,
// чтобы переключение дальше шло мгновенно.
function warmUpPages() {
  const idle = window.requestIdleCallback
    ? (cb) => requestIdleCallback(cb, { timeout: 4000 })
    : (cb) => setTimeout(cb, 1500);
  idle(() => {
    try {
      // если пользователь уже переключает вкладку — не мешаем, прогреем позже
      if (document.querySelector('.page.leaving, .page.entering')) { warmUpPages(); return; }
      $$('.page').forEach((pg) => {
        const was = pg.classList.contains('active');
        if (!was) pg.classList.add('active');
        void pg.offsetHeight; // форсируем построение layout/стилей
        if (!was) pg.classList.remove('active');
      });
      ['account', 'mods', 'link'].forEach((p) => { try { refreshPageSoon(p); } catch (e) {} });
    } catch (e) {}
  });
}
warmUpPages();
