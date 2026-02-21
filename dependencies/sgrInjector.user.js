// ==UserScript==
// @name        sgrInjector
// @namespace   Violentmonkey Scripts
// @match       https://bonk.io/*
// @run-at      document-start
// @grant       none
// @version     1.0
// @author      StarCubey
// @description An injector that can run at any time without being limited to document-start and game frame.
// ==/UserScript==

if(window.location.toString() !== "https://bonk.io/sgr") return;
const mods = ["sgrAPI"];
window.sgrMods = {};

let frame = undefined;
let resolveGameFrame;
window.gameFrame = new Promise(res => resolveGameFrame = () => res(frame));

let resolveGameFrameLoaded;
window.gameFrameLoaded = new Promise(res => resolveGameFrameLoaded = () => res(frame));

let html = new Promise(async res => {
  let html = await (await fetch("https://bonk.io/")).text();
  html = html.replace(`<iframe id="maingameframe" src="gameframe-release.html"></iframe>`, `<iframe id="maingameframe"></iframe>`);
  res(html);
});

let alpha2s = new Promise(async res => {
  let code = await (await fetch("js/alpha2s.js")).text();
  code = code
    .replaceAll(/if\(!\((?:[A-Za-z0-9\$_]{3}\.[A-Za-z0-9\$_]{3}\(\d+,false,\d+\)\s!==\s[A-Za-z0-9\$_]{3}\[\d+\]\s&&\s)+[A-Za-z0-9\$_]{3}\.[A-Za-z0-9\$_]{3}\(\d+,false,\d+\)\s!==\s[A-Za-z0-9\$_]{3}\[\d+\]\)\)/g, "if(true)")
    .replaceAll(/if\((?:[A-Za-z0-9\$_]{3}\.[A-Za-z0-9\$_]{3}\(\d+,false,\d+\)\s===\s[A-Za-z0-9\$_]{3}\[\d+\]\s\|\|\s)+[A-Za-z0-9\$_]{3}\.[A-Za-z0-9\$_]{3}\(\d+,false,\d+\)\s===\s[A-Za-z0-9\$_]{3}\[\d+\]\)/g, "if(true)");

  for(mod of mods) {
    let loaded = false;
    for(let i = 0; i < 100; i++) {
      if(window.sgrMods[mod]) {
        code = window.sgrMods[mod](code);
        console.log(mod + " injected");
        loaded = true;
        break;
      }
      await new Promise(res => setInterval(res, 100));
    }
    if(!loaded) console.log("Failed to load "+mod);
  }

  res(btoa(code + "window.resolveGameFrameLoaded();"));
});

let requirejs = new Promise(async res => {
  let code = await (await fetch("js/require.js")).text();
  alpha2s = await alpha2s;
  code = code.replace(`e.src=r`, `r==="js/alpha2s.js"?(e.textContent=atob("${alpha2s}"),e.dispatchEvent(new Event("load"))):e.src=r`);
  res(code);
});

let frameHtml = new Promise(async res => {
  let html = await (await fetch("gameframe-release.html")).text();
  requirejs = await requirejs;
  html = html
    .replace(`<script data-main="js/alpha2s" src="js/require.js"></script>`, `<script data-main="js/alpha2s">${requirejs}</script>`)
  res(html);
});

(async () => {
  html = await html;
  document.documentElement.innerHTML =
    (new DOMParser()).parseFromString(html, "text/html").documentElement.innerHTML;
  frame = document.getElementById("maingameframe");
  resolveGameFrame();

  frame.srcdoc = await frameHtml;
  frame.addEventListener("load", () => frame.contentWindow.resolveGameFrameLoaded = resolveGameFrameLoaded);
})();

// Condensed Injector (copied from Excigma's code injector)

// let url = "https://bonk.io/js/alpha2s.js";
// window.code = new Promise(async res => res(await (await fetch(url + "?")).text()));
// window._appendChild = document.head.appendChild;
// document.head.appendChild = function(...args) {
//   if(args[0].src === url) {
//     args[0].removeAttribute("src");
//     (async () => {
//       window.code = await window.code;
//       while(!window.bonkCodeInjectors) await new Promise(res => setTimeout(res, 100));
//       for(injector of window.bonkCodeInjectors) window.code = injector(window.code);
//       args[0].textContent = window.code;
//       args[0].dispatchEvent(new Event("load"));
//       return window._appendChild.apply(document.head, args);
//     })();
//   } else {
//     return window._appendChild.apply(document.head, args);
//   }
// }
