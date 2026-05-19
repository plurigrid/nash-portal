# ◈

## Color legend

Proper names in this codebase resolve through `SplitMix64(seed = 1069) → HSL(0.75, 0.55) → RGB`:

| was | sigil | hex |
|---|---|---|
| NASH | ◆ | `#E23C36` |
| SOL / POOL | ◇ | `#36E2A3` |
| pump.fun | ✦ | `#E23653` |
| GeckoTerminal | ⬢ | `#E23667` |
| Jupiter | ✧ | `#AC36E2` |
| DexScreener | ✿ | `#C836E2` |
| Plurigrid | ⬡ | `#E25936` |
| nash-portal / NASH Portal | ◈ | `#E27B36` |

## Post-web thesis

The browser was the wrong cathedral. It fused identity, rendering, capability, and payment into a single opaque binary governed by ad-tech. `◈` is built on the inverse premise: **one UI, two surfaces, no backend, no secrets**.

- **Rendering is a commodity.** The same ratatui widget tree paints a terminal (`tui/`) and a WASM canvas (`web/`). Neither surface is privileged; the TUI is the reference.
- **Data is public or it is not data.** OHLCV streams from `⬢`'s public API. No keys, no auth, no server-side state. If a feed needs a secret, it doesn't belong in the portal.
- **Identity is on-chain or it is off-channel.** No accounts, no sign-ins, no cookies. Wallet facts (Neversold predicate, Tier-S exclusion, Scheme-C eligibility) are computed from the chain, not claimed by a server.
- **The portal is a lens, not a platform.** It does not custody, broker, or route. It renders a token's public state and the cohort-level consequences of the Neversold predicate.
- **Releases are artifacts, not services.** `gh release` ships three signed tarballs and one WASM bundle. Uptime is a property of Cloudflare Pages + the user's terminal; there is nothing to operate.

Post-web means: the useful half of a web app (pixels on a grid, live data, a link you can share) without the extractive half (the session, the login, the tracker, the middleman).

## Surfaces

- **web** — [ratzilla](https://github.com/orhun/ratzilla) WASM build, deployed to Cloudflare Pages
- **tui** — native [ratatui](https://github.com/ratatui/ratatui) binary for macOS / Linux

OHLCV candles stream from the public `⬢` API. No keys, no backend.

## Layout

```
◈/
├── Cargo.toml      # workspace: [web, tui]
├── web/            # ratzilla 0.3 → wasm32-unknown-unknown, built with trunk
└── tui/            # ratatui 0.30 + crossterm 0.28, native
```

`ratatui 0.30` is required so the workspace's `unicode-width` resolves against ratzilla 0.3 (which needs `^0.2.2`).

## Run

**Web (local):**
```
cd web && trunk serve --release
```

**TUI:**
```
cd tui && cargo run --release
```

## Build & release

CI (`.github/workflows/`) handles:

- `trunk build --release` → upload `web/dist/` → `wrangler pages deploy --project-name nash-portal`
- Cross-compile TUI for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`
- On `v*` tags: `gh release create` with all three tarballs attached

Required secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`.

(The Cloudflare project name `nash-portal` is preserved as a functional identifier — external infrastructure config.)

## Token

`◆` mint: `4DQsMSkeKc3Mcij1BE4Z8oqU3QeV45QQ3Psn3CNDpump` (`✦`, ossified).

Neversold cohort and Scheme-C forfeiture detail: see release notes on each `v*` tag.
