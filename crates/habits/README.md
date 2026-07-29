# habits

Tap-to-track habit PWA built with Dioxus (web/wasm). Fully client-side:
habits and their timestamped ticks live in the browser's localStorage, so
the app works offline — no server, no account, no sync.

Each card grows a thin line along its bottom edge as the habit approaches
66 days' worth of strength — the median time to automaticity found by
Lally et al. (2010) and confirmed by a 2024 meta-analysis (median 59–66
days). Every practiced day adds a day of strength; a single missed day is
free (occasional misses don't affect formation), but each further
consecutive idle day erodes half a day.

Each card's ▦ button opens a month calendar shaded by how often the habit
was done each day. Selecting a day shows its count with a − / + stepper:
today and the previous 7 days can be corrected there (forgot to log,
logged twice, ...); older days are view-only.

## Develop

```sh
dx serve            # from crates/habits; hot-reloading dev server
```

`dx` is in the devenv shell. Unit tests for the date math run natively:
`cargo test -p habits`.

## Ship

Offline launch needs a service worker, and browsers only register those on
HTTPS origins (localhost is exempt, LAN IPs are not). So: build, drop the
`web/` files into the output root, host the result on any static HTTPS host
(GitHub Pages, Netlify, a homelab behind a real cert, ...):

```sh
dx build --release
cp web/* target/dx/habits/release/web/public/
```

Then open the URL on the phone once, and "Add to Home Screen". From then on
it launches and works with no connectivity.

Caveats of being fully client-side: data is per-device (no sync), and
clearing the browser's site data deletes it.
