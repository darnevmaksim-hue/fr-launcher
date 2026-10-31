// ===== Панель «Ресурсы» на главной странице =====
// Показывает пики/средние RAM и CPU последней сессии, спарклайн памяти и
// корреляцию «изменил моды — вырос расход» (данные из src/metrics.js через
// IPC metrics:history). Обновляется при старте, при возврате на главную и
// сразу после закрытия игры.
(function () {
  const root = document.getElementById('metrics-home');
  if (!root || !window.API || !window.API.metricsHistory) return;

  const q = (s, c) => (c || document).querySelector(s);
  const qa = (s, c) => Array.from((c || document).querySelectorAll(s));

  function esc(s) {
    return String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }
  function fmtMb(v) {
    v = Math.max(0, Math.round(v || 0));
    return v >= 1024 ? (v / 1024).toFixed(1) + ' ГБ' : v + ' МБ';
  }
  function fmtDur(ms) {
    ms = Math.max(0, ms | 0);
    const h = Math.floor(ms / 3600000);
    const m = Math.floor(ms / 60000) % 60;
    if (h) return h + ' ч ' + m + ' мин';
    if (m) return m + ' мин';
    return Math.floor(ms / 1000) + ' с';
  }
  function fmtWhen(ts) {
    const d = new Date(ts);
    const p = (n) => String(n).padStart(2, '0');
    return p(d.getDate()) + '.' + p(d.getMonth() + 1) + ' ' + p(d.getHours()) + ':' + p(d.getMinutes());
  }
  function card(lbl, val, sub) {
    return '<div class="metric-card"><div class="m-lbl">' + lbl + '</div>' +
      '<div class="m-val">' + val + '</div>' +
      (sub ? '<div class="m-sub">' + sub + '</div>' : '') + '</div>';
  }

  // Спарклайн: RAM последней сессии по времени (SVG, viewBox масштабируется)
  function sparkline(samples) {
    const pts = (samples || []).filter((s) => s && typeof s.ramMb === 'number' && s.ramMb > 0);
    if (pts.length < 2) return '';
    const W = 300, H = 54, P = 4;
    const vals = pts.map((s) => s.ramMb);
    const min = Math.min.apply(null, vals);
    const max = Math.max.apply(null, vals);
    const span = Math.max(1, max - min);
    const coords = pts.map((s, i) => {
      const x = P + (i / (pts.length - 1)) * (W - P * 2);
      const y = H - P - ((s.ramMb - min) / span) * (H - P * 2);
      return x.toFixed(1) + ',' + y.toFixed(1);
    });
    return '<div class="metric-spark">' +
      '<div class="metric-spark-lbl">Память по времени · ' + fmtMb(min) + ' → ' + fmtMb(max) + '</div>' +
      '<svg viewBox="0 0 ' + W + ' ' + H + '" preserveAspectRatio="none" aria-hidden="true">' +
      '<polyline points="' + coords.join(' ') + '" fill="none" stroke="var(--ac)" stroke-width="1.6" ' +
      'stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>' +
      '</svg></div>';
  }

  function analysisBlock(an) {
    if (!an) return '';
    const sign = (v) => (v >= 0 ? '+' : '') + v;
    let html = '<div class="metric-analysis">Сравнение с прошлой сессией: <b>' +
      sign(an.deltaRamMb) + ' МБ RAM</b>, <b>' + sign(an.deltaCpu) + '% CPU</b>';
    if (an.added && an.added.length) html += ', добавлено: <b>' + an.added.map(esc).join(', ') + '</b>';
    if (an.removed && an.removed.length) html += ', удалено: ' + an.removed.map(esc).join(', ');
    html += '<span class="ma-hint">' + esc(an.hint) + '</span></div>';
    return html;
  }

  function render(data) {
    const sessions = (data && data.sessions) || [];
    if (!sessions.length) {
      root.innerHTML = '<div class="metric-empty">Метрика появится после первой сессии — запусти игру, ' +
        'поиграй и закрой: здесь будут пики и средние по RAM/CPU.</div>';
      return;
    }
    const last = sessions[0];
    const totalMs = sessions.reduce((p, c) => p + (c.durationMs || 0), 0);
    const sub = sessions.length;
    let html = '<div class="metric-cards">';
    html += card('Последняя сессия', fmtDur(last.durationMs), esc(last.name) + ' · ' + fmtWhen(last.startedAt));
    html += card('Пик RAM', fmtMb(last.peakRamMb), last.memMaxMb ? 'лимит ' + fmtMb(last.memMaxMb) : 'за сессию');
    html += card('Средняя RAM', fmtMb(last.avgRamMb), 'за сессию');
    html += card('Пик CPU', String(last.peakCpu == null ? 0 : last.peakCpu) + '%', 'от всех ядер');
    html += card('Средняя CPU', String(last.avgCpu == null ? 0 : last.avgCpu) + '%', 'за сессию');
    html += card('Всего наиграно', fmtDur(totalMs), 'сессий: ' + sub);
    html += '</div>';
    html += sparkline(last.samples);
    html += analysisBlock(data.analysis);
    root.innerHTML = html;
  }

  let refreshTimer = null;
  function refresh() {
    if (refreshTimer) return;
    refreshTimer = setTimeout(() => { refreshTimer = null; }, 400);
    window.API.metricsHistory()
      .then(render)
      .catch(() => {});
  }

  // обновление: сразу, при клике на «Главная» и после закрытия игры
  refresh();
  qa('.nav-item').filter((b) => b.dataset.page === 'home')
    .forEach((b) => b.addEventListener('click', refresh));
  if (window.API.onStatsChanged) window.API.onStatsChanged(refresh);
  if (window.API.onGameLog) {
    window.API.onGameLog((d) => {
      if (d && /Game closed/.test(d.text || '')) setTimeout(refresh, 1500);
    });
  }
})();
