const CACHE = 'website'
const assets = ['/']

self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE).then(cache =>
        cache.addAll(assets))
    )
})

self.addEventListener('fetch', function (event) {
    const response = fromCache(event.request) // try cache
        .then(response => response)
        .catch(() => fromServer(event.request) // try server
            .then(response => response).catch(() => fallback()) // generic fallback
        )
    event.respondWith(response)
    event.waitUntil(update(event.request))
})

function fromCache (request) {
    return new Promise((resolve, reject) => {
        caches.open(CACHE).then(cache => {
            cache.match(request)
                .then(matching => {
                    if (matching) resolve(matching)
                    else reject('no-match')
                })
                .catch(() => reject('no-match'))
        })
  })
}

function fromServer (request) {
    return fetch(request)
}

function fallback () {
    // If both fail, show a generic fallback:
    return caches.match('/')
}

function update (request) {
    return caches.open(CACHE).then(cache =>
        fetch(request).then(response =>
            cache.put(request, response)
        ).catch(() => {})
    ).catch(() => {})
}