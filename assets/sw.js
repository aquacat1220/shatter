var cacheName = 'shatter-cache';

/* Serve cached content when offline */
self.addEventListener('fetch', function (e) {
  e.respondWith(
    fetch(e.request)
      .then(function (response) {
        return response;
      })
      .catch(function () {
        return caches.match(e.request);
      })
  );
});
