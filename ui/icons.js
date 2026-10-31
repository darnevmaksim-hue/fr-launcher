/* ============================================================
   FR-Launcher - inline SVG icon set
   24x24 grid, stroke-based line icons, engineering style
   ------------------------------------------------------------
   Each value below is the INNER markup of an <svg viewBox="0 0 24 24">.
   Fragments carry NO fill / stroke / width / height / stroke-width of
   their own, so they inherit everything from the wrapper, e.g.:

     <svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
          stroke-width="1.6" stroke-linecap="round"
          stroke-linejoin="round" aria-hidden="true">
       <!-- ICONS.home -->
     </svg>

   Self-contained. No dependencies. No emoji, no gradients, no fills.
   ============================================================ */
(function (root) {
  'use strict';

  var ICONS = {
    /* --- primary navigation ------------------------------------------- */
    home:
      '<path d="M3.5 10.4 12 3.6l8.5 6.8"/>' +
      '<path d="M5.6 9.5V19a1.6 1.6 0 0 0 1.6 1.6h9.6A1.6 1.6 0 0 0 18.4 19V9.5"/>' +
      '<path d="M9.6 20.6v-5.4h4.8v5.4"/>',

    mods:
      '<path d="M12 3.2 21 8l-9 4.8L3 8z"/>' +
      '<path d="M3 12.5 12 17.3l9-4.8"/>' +
      '<path d="M3 16.5 12 21.3l9-4.8"/>',

    link:
      '<path d="M10.5 13.5a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1.7 1.7"/>' +
      '<path d="M13.5 10.5a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1.7-1.7"/>',

    account:
      '<circle cx="12" cy="7.6" r="3.8"/>' +
      '<path d="M4.6 20.4a7.4 7.4 0 0 1 14.8 0"/>',

    /* --- playback / actions ------------------------------------------- */
    play:
      '<path d="M7 4.8 19 12 7 19.2z"/>',

    stop:
      '<rect x="6" y="6" width="12" height="12" rx="2"/>',

    refresh:
      '<path d="M4 12a8 8 0 0 1 13.3-6"/>' +
      '<path d="M17.5 2.8v3.4h-3.4"/>' +
      '<path d="M20 12a8 8 0 0 1-13.3 6"/>' +
      '<path d="M6.5 21.2v-3.4h3.4"/>',

    /* --- file / system ------------------------------------------------ */
    folder:
      '<path d="M3.5 7A2 2 0 0 1 5.5 5h3.4a1 1 0 0 1 .8.4L11 7.2h7.5A2 2 0 0 1 20.5 9.2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/>',

    settings:
      '<circle cx="12" cy="12" r="3.2"/>' +
      '<path d="M12 3.2v2.4"/><path d="M12 18.4v2.4"/>' +
      '<path d="M3.2 12h2.4"/><path d="M18.4 12h2.4"/>' +
      '<path d="M5.9 5.9 7.6 7.6"/><path d="M16.4 16.4l1.7 1.7"/>' +
      '<path d="M18.1 5.9 16.4 7.6"/><path d="M7.6 16.4l-1.7 1.7"/>',

    trash:
      '<path d="M4 6.5h16"/>' +
      '<path d="M9 6.5V4.8A1.3 1.3 0 0 1 10.3 3.5h3.4A1.3 1.3 0 0 1 15 4.8V6.5"/>' +
      '<path d="M6.5 6.5 7.4 19.6A1.6 1.6 0 0 0 9 21.1h6a1.6 1.6 0 0 0 1.6-1.5l.9-13.1"/>' +
      '<path d="M10.2 10.5v6.5"/><path d="M13.8 10.5v6.5"/>',

    upload:
      '<path d="M12 15.5V4"/>' +
      '<path d="M7 9 12 4l5 5"/>' +
      '<path d="M4.5 19.5h15"/>',

    download:
      '<path d="M12 4v11.5"/>' +
      '<path d="M7 10.5 12 15.5l5-5"/>' +
      '<path d="M4.5 19.5h15"/>',

    /* --- search / status ---------------------------------------------- */
    search:
      '<circle cx="10.5" cy="10.5" r="6.5"/>' +
      '<path d="M15.3 15.3 21 21"/>',

    close:
      '<path d="M5.5 5.5 18.5 18.5"/>' +
      '<path d="M18.5 5.5 5.5 18.5"/>',

    check:
      '<path d="M4.5 12.5 9.5 17.5 20 6.5"/>',

    alert:
      '<path d="M12 4.2 21 19.5H3z"/>' +
      '<path d="M12 10v4.2"/>' +
      '<circle cx="12" cy="16.9" r="0.7"/>',

    info:
      '<circle cx="12" cy="12" r="8.5"/>' +
      '<path d="M12 11v5.5"/>' +
      '<circle cx="12" cy="7.7" r="0.7"/>',

    /* --- chevrons / arrows -------------------------------------------- */
    chevron:
      '<path d="M6 9.5 12 15.5 18 9.5"/>',

    chevronUp:
      '<path d="M6 14.5 12 8.5 18 14.5"/>',

    chevronLeft:
      '<path d="M14.5 6 8.5 12 14.5 18"/>',

    chevronRight:
      '<path d="M9.5 6 15.5 12 9.5 18"/>',

    arrowRight:
      '<path d="M4.5 12h15"/>' +
      '<path d="M13.5 6 19.5 12l-6 6"/>',

    arrowLeft:
      '<path d="M19.5 12h-15"/>' +
      '<path d="M10.5 6 4.5 12l6 6"/>',

    external:
      '<path d="M14 4.5h5.5V10"/>' +
      '<path d="M19.5 4.5 11 13"/>' +
      '<path d="M18 13.5v5A1.5 1.5 0 0 1 16.5 20h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6h5"/>',

    /* --- plus / minus / misc ------------------------------------------ */
    plus:
      '<path d="M12 4.5v15"/>' +
      '<path d="M4.5 12h15"/>',

    minus:
      '<path d="M4.5 12h15"/>',

    menu:
      '<path d="M4 7h16"/>' +
      '<path d="M4 12h16"/>' +
      '<path d="M4 17h16"/>',

    dots:
      '<circle cx="6" cy="12" r="1"/>' +
      '<circle cx="12" cy="12" r="1"/>' +
      '<circle cx="18" cy="12" r="1"/>',

    /* --- domain / launcher specific ----------------------------------- */
    java:
      '<path d="M4.5 8.5h12v5.5a4.5 4.5 0 0 1-4.5 4.5H9a4.5 4.5 0 0 1-4.5-4.5z"/>' +
      '<path d="M16.5 9.5h1.6a2.4 2.4 0 0 1 0 4.8h-1.6"/>' +
      '<path d="M7.5 3.5c-.9 1-.9 2.1 0 3.1"/>' +
      '<path d="M11 3.5c-.9 1-.9 2.1 0 3.1"/>',

    cube:
      '<path d="M12 3 20 7.5v9L12 21 4 16.5v-9z"/>' +
      '<path d="M4 7.5 12 12l8-4.5"/>' +
      '<path d="M12 12v9"/>',

    shield:
      '<path d="M12 3 19 5.6v5.2c0 4.4-2.9 7.9-7 9.7-4.1-1.8-7-5.3-7-9.7V5.6z"/>',

    spark:
      '<path d="M12 3.5 13.6 9 19 10.5 13.6 12 12 17.5 10.4 12 5 10.5 10.4 9z"/>' +
      '<path d="M18.6 15.4l.5 1.7 1.7.5-1.7.5-.5 1.7-.5-1.7-1.7-.5 1.7-.5z"/>',

    globe:
      '<circle cx="12" cy="12" r="8.5"/>' +
      '<path d="M3.5 12h17"/>' +
      '<path d="M12 3.5a13 13 0 0 1 0 17 13 13 0 0 1 0-17z"/>',

    clock:
      '<circle cx="12" cy="12" r="8.5"/>' +
      '<path d="M12 7.5V12l3.2 2"/>',

    star:
      '<path d="M12 3.8l2.6 5.3 5.8.8-4.2 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.2-4.1 5.8-.8z"/>',

    filter:
      '<path d="M3.5 5.5h17l-6.6 7.7v5.3l-3.8 2v-7.3z"/>',

    lock:
      '<rect x="5" y="10.5" width="14" height="9.5" rx="2"/>' +
      '<path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/>'
  };

  /* Attach to the global object (renderer / browser) ... */
  root.ICONS = ICONS;

  /* ... and to CommonJS when imported from Node (for tests/validation). */
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = ICONS;
  }
})(typeof window !== 'undefined' ? window : globalThis);
