// NOVA static site worker: cache the shell, fall back to it offline.
// Bump this whenever a cached file changes. The service worker serves the old
// copy until the name moves, so a content edit without a bump is invisible to
// anyone who has already visited.
const CACHE = "nova-site-v5";
const SHELL = [
  "./",
  "./index.html",
  "./404.html",
  "./manifest.webmanifest",
  "./language/",
  "./language/index.html",
  "./language/00_overview.md",
  "./language/01_lexical_grammar.md",
  "./language/02_tokens.md",
  "./language/03_keywords.md",
  "./language/04_identifiers.md",
  "./language/05_literals.md",
  "./language/06_types.md",
  "./language/07_functions.md",
  "./language/08_modules.md",
  "./language/09_control_flow.md",
  "./language/10_ownership.md",
  "./language/11_memory.md",
  "./language/12_errors.md",
  "./language/13_concurrency.md",
  "./language/14_attributes.md",
  "./language/15_generics.md",
  "./language/16_interfaces.md",
  "./language/17_packages.md",
  "./language/18_domain_packs.md",
  "./language/19_semantic_model.md",
  "./language/20_sir_intro.md",
  "./language/21_style_guide.md",
  "./language/22_best_practices.md",
  "./language/23_glossary.md",
  "./language/24_rfc_process.md",
  "./assets/favicon.svg",
  "./assets/icon-192.png",
  "./assets/icon-512.png",
  "./assets/logo.svg",
  "./assets/og.svg",
  "./assets/support-portrait.jpg",
  "./assets/upi-qr.svg",
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(SHELL)).then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET" || new URL(request.url).origin !== location.origin) return;

  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request)
        .then((response) => {
          const copy = response.clone();
          caches.open(CACHE).then((cache) => cache.put(request, copy));
          return response;
        })
        .catch(() => caches.match(request).then((hit) => hit || caches.match("./index.html")))
    );
    return;
  }

  event.respondWith(
    caches.match(request).then((hit) => {
      if (hit) return hit;
      return fetch(request).then((response) => {
        if (response.ok) {
          const copy = response.clone();
          caches.open(CACHE).then((cache) => cache.put(request, copy));
        }
        return response;
      });
    })
  );
});
