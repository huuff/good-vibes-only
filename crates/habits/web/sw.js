// Stale-while-revalidate: serve from cache immediately (works offline),
// refresh the cache in the background. dx hashes asset filenames, so a new
// deploy's index.html pulls new URLs and old entries just go stale.
const CACHE = "habits-v1";

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(["./"]))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
  if (event.request.method !== "GET") return;
  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE);
      const cached = await cache.match(event.request);
      const network = fetch(event.request)
        .then((response) => {
          if (response.ok) cache.put(event.request, response.clone());
          return response;
        })
        .catch(() => undefined);
      return (
        cached ||
        (await network) ||
        (event.request.mode === "navigate" ? cache.match("./") : Response.error())
      );
    })(),
  );
});
