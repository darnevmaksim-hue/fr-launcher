/* Мост между нашим интерфейсом (renderer.js) и Rust-ядром.
   Повторяет контракт preload.js из Electron-версии: window.API.<метод>(...). */
(function () {
  var tauri = window.__TAURI__;
  if (!tauri || !tauri.core) {
    console.error('[shim] Tauri API недоступен — интерфейс не сможет работать');
    return;
  }
  var invoke = tauri.core.invoke;

  function call(method) {
    return function () {
      var args = Array.prototype.slice.call(arguments);
      return invoke('api', { method: method, args: args }).catch(function (e) {
        console.error('[shim]', method, e);
        throw e;
      });
    };
  }

  function on(event, cb) {
    var unlisten = null;
    var cancelled = false;
    tauri.event
      .listen(event, function (e) {
        cb(e.payload);
      })
      .then(function (fn) {
        unlisten = fn;
        if (cancelled) fn();
      });
    return function () {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }

  window.API = {
    appInfo: call('app:info'),
    exists: call('fs:exists'),
    openDialog: call('dialog:open'),

    // Java
    detectJava: call('java:detect'),
    ensureJava: call('java:ensure'),
    onJavaLog: function (cb) {
      return on('java:log', cb);
    },

    // Microsoft account
    msStatus: call('ms:status'),
    msStart: call('ms:start'),
    msPoll: call('ms:poll'),
    msAuth: call('ms:auth'),
    msLogout: call('ms:logout'),
    onAccountChanged: function (cb) {
      return on('account:changed', cb);
    },

    // Minecraft
    mcVersions: call('mc:versions'),
    mcDir: call('mc:dir'),

    // Loaders
    fabricVersions: call('loader:fabric:versions'),

    // Instances
    instancesList: call('instances:list'),
    instanceDir: call('instances:dir'),
    instanceCreate: call('instances:create'),
    instanceDelete: call('instances:delete'),
    instanceSaveMods: call('instances:saveMods'),
    instanceMods: call('instance:mods'),
    instanceModDelete: call('instance:modDelete'),
    instanceModDeleteFile: call('instance:modDeleteFile'),
    instanceFiles: call('instance:files'),
    instanceFileDelete: call('instance:fileDelete'),

    // Modrinth
    modrinthSearch: call('modrinth:search'),
    modrinthProject: call('modrinth:project'),
    modrinthVersions: call('modrinth:versions'),
    openExternal: call('open:external'),
    openPath: call('open:path'),
    modInstall: call('mod:install'),
    modInstallDirect: call('mod:installDirect'),
    modDeps: call('mod:deps'),
    modpackInstall: call('modpack:install'),

    // Launch
    launch: call('game:launch'),
    onGameLog: function (cb) {
      return on('game:log', cb);
    },
    onProgress: function (cb) {
      return on('prog', cb);
    },

    // Customization
    pickBg: call('ui:pickBg'),
    getBg: call('ui:getBg'),
    setBg: call('ui:setBg'),
    clearBg: call('ui:clearBg'),

    // Network
    netCheck: call('net:check'),

    // Link (хаб лаунчеров)
    linkPick: call('link:pick'),
    linkExtractIcon: call('link:icon'),
    linkLaunch: call('link:launch'),
    linkTabLaunch: call('link:tabLaunch'),
    linkTabSwitch: call('link:tabSwitch'),
    linkTabClose: call('link:tabClose'),
    linkTabDetach: call('link:tabDetach'),
    linkTabList: call('link:tabList'),
    linkStats: call('link:stats'),
    linkStatsReset: call('link:statsReset'),
    onEmbedsChanged: function (cb) {
      return on('embed:changed', cb);
    },

    // LAN
    lanHostInfo: call('lans:hostInfo'),
    lanJoin: call('lans:join'),
    lanLeave: call('lans:leave'),
    lanStatus: call('lans:status'),

    // Sync
    syncStatus: call('sync:status'),
    syncEnable: call('sync:enable'),
    syncDisable: call('sync:disable'),
    syncPull: call('sync:pull'),
    syncAnnounce: call('sync:announce'),
    onSyncChanged: function (cb) {
      return on('sync:changed', cb);
    },

    // Auto-update
    updateCheck: call('autoUpdate:check'),
    updateDownload: call('autoUpdate:download'),
    updateInstall: call('autoUpdate:install'),
    updateSkip: call('autoUpdate:skip'),
    updateClearSkip: call('autoUpdate:clearSkip'),

    // Stats
    statsGet: call('stats:get'),
    statsReset: call('stats:reset'),
    onStatsChanged: function (cb) {
      return on('stats:changed', cb);
    },

    // Metrics
    metricsHistory: call('metrics:history'),
    metricsHistoryFor: call('metrics:historyFor'),

    // Diagnostics
    depsAnalyze: call('deps:analyze'),
    repairScan: call('repair:scan'),
    repairRun: call('repair:run'),
    onGameCrash: function (cb) {
      return on('game:crash', cb);
    },
  };

  // Frameless-окно: тянем за верхнюю панель (аналог -webkit-app-region: drag).
  try {
    var win = tauri.window.getCurrentWindow();
    document.addEventListener('DOMContentLoaded', function () {
      var bar = document.querySelector('.topbar');
      if (!bar) return;
      bar.style.cursor = 'default';
      bar.addEventListener('mousedown', function (e) {
        if (e.button !== 0) return;
        var el = e.target;
        while (el && el !== bar) {
          if (
            el.tagName === 'BUTTON' ||
            el.tagName === 'INPUT' ||
            el.tagName === 'A' ||
            el.tagName === 'SELECT' ||
            el.tagName === 'TEXTAREA' ||
            (el.classList && (el.classList.contains('mini-acct') || el.classList.contains('nav-item')))
          ) {
            return;
          }
          el = el.parentElement;
        }
        win.startDragging();
      });
    });
  } catch (e) {
    console.warn('[shim] drag region недоступен', e);
  }
})();
