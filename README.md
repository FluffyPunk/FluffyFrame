# FluffyFrame

FluffyFrame is a Rust port of [WFHelper](https://github.com/WFHelper/WFHelper), the unofficial
Warframe companion app. It is a native egui app with no Electron and no Node.

## Comparison

| Stats | AlecaFrame | WFHelper | FluffyFrame |
|---|---|---|---|
| System availability | Windows only | Windows and Linux AppImage | Windows and Linux Binary |
| Code solution | Proprietary Overwolf plugin | Open-source Electron App | Open-source Rust Binary |
| RAM usage | Idk, but consider that it also uses Overwolf itself | 400MB-1GB | 100-200MB(current release) |
| Features | Relic and Riven Overlays | Same, but lacking language support | Reroll analyzer(API level, no language dependency), other WIP |

## Run

There are portable binaries on [releases](https://github.com/FluffyPunk/FluffyFrame/releases). Just download and open.

```sh
cargo run --release
```

On Linux, reading the game's memory is blocked by Yama `ptrace_scope` on most
distributions. To allow it, do one of these:

```sh
sudo setcap cap_sys_ptrace+ep target/release/fluffyframe   # per binary
sudo sysctl kernel.yama.ptrace_scope=0                     # until reboot
```

If you can't allow it, import an `inventory.json` or an AlecaFrame `lastData.dat`
in Settings instead.

Environment overrides (the pre-rename `WFHELPER_*` names still work):

| Variable | Purpose |
|---|---|
| `FLUFFYFRAME_DATA_DIR` | Data folder (settings, snapshot, caches). Defaults to the platform data dir; a pre-rename `wfhelper-rust` folder there is moved over on first launch. |
| `FLUFFYFRAME_EE_LOG` | EE.log path, overriding discovery. |
| `FLUFFYFRAME_DEBUG` | Debug logging. |
| `FLUFFYFRAME_VIEW` | Opens a specific view on launch (`inventory`, `relics`, …), handy for screenshots. |

## Layout

The structure follows the original repository:

| Original | Here |
|---|---|
| `main.ts` (Electron bootstrap) | `src/main.rs`, `src/app.rs` |
| `ipc/*` + main process | `src/ipc/`: worker threads (data, inventory, prices, market, EE.log router) |
| `src/views/*.svelte` | `src/views/*.rs` |
| `src/components/*` | `src/components/` |
| `src/stores/*` | `src/stores/` |
| `src/styles/tokens.css`, `src/lib/theme/*` | `src/theme.rs` (same color tokens, Rajdhani/Barlow fonts) |
| `src/lib/assetUrls.ts`, `assets/` | `src/assets.rs`, `assets/` (embedded at build time) |
| `config/*` | `crates/fluffyframe-core/src/config/` |
| `services/*` | `crates/fluffyframe-core/src/services/` (each file names the TS file it replaces) |
| warframe-api-helper download + `gameMemory*.ts` | `crates/wf-api`: [wf-api-rust](https://github.com/FluffyPunk/wf-api-rust) as a library |

`crates/wf-api` keeps wf-api-rust's `process.rs` verbatim and its scan algorithm,
returning typed errors in place of exit codes. wf-api-rust is binary-only, so it
is vendored here. If it gains a `lib.rs`, this crate can become a git dependency.

### Data sources

- DE public export (`content.warframe.com/PublicExport`): items, recipes, relics,
  nodes and icons, cached per manifest hash.
- DE world state (`api.warframe.com/cdn/worldState.php`), parsed locally.
- warframe.market v1/v2: catalog, 48h closed-volume average prices, order books,
  your orders, and riven auctions. Requests are rate-limited to about 3 per second.
- browse.wf `arbys.txt` for the arbitration schedule.
- drops.warframestat.us for drop locations.

## Assets

`assets/` holds the original's icons, planet and relic art (from the MIT-licensed
WFHelper repo; game art belongs to Digital Extremes) and the Rajdhani and Barlow
fonts (SIL OFL, licenses in `assets/fonts/`), converted from WOFF2 to TTF because
egui reads TTF/OTF only. The Rajdhani files have the original's `.03em` letter
spacing built into their advance widths, since egui has no global letter spacing. Everything is embedded in the binary.

## Feature status

Ported:

- **Dashboard**: widget cards for cycles, fissures, Baro, wallet, est. value,
  foundry, trades and the current arbitration

- **Inventory**: card grid or rows, category tabs, include chips, value
  estimate, full sets, the Market Listings side panel (best WTS/WTB/spread,
  rank view, sort, Post WTS/WTB, whisper/profile per order), bulk sell
  
- **Item detail modal** from any view: tags (Prime, Vaulted, Mastered, Parent
  owned), "Part of" navigation with back stack, colored components list with a
  side panel per component, drops, relics, crafting tree
  
- **Relic Planner**: cards with owned-reward highlighting, and a detail modal
  with refinement tabs and Solo/2P/3P/4P expected value (feature diff: proper name sorting, WIP)

- **Foundry** (filters, cards open the item modal), **Mastery** (roadmap,
  detailed breakdown, collection grid), **Stats**, **Syndicates**, **Market**,
  **Wiki / Drop Data**, **Settings**, **Setup** wizard
  
- **Rivens**: fully implemented Riven gallery: rolls, stats, grading, market comparison and listing. Also implemented API-level reroll analyzer.
> [!NOTE]
> For those, who kinda know and who doesn't at all - Original project(WFHelper) uses OCR for reading stats from both sides of reroll to compare. Pros: time, cons: accuracy. I(using Claude ofc) found a spot in inventory API response, when reroll is pending. Pros: 100% accuracy, cons: no instant marker, so either delayed automated API request or manual request by hotkey.
 
- **World**: revamped dashboard about all the general stats: planet cycles, reset timers, invasion, alerts, Circuit etc.
Unique solutions: invirogations forecast(same as browse.wf)

- All 16 theme presets from the original, plus corner style

- EE.log tailing (Steam/Proton, Heroic, Lutris, Bottles, Wine, Windows) for
  logins, whispers and trades, with desktop notifications for whispers, trades,
  favorite arbitrations and fissure rules

WIP:

- all in-game overlays (relic rewards, relic planner, riven, trade notification)
- Run analyzers - Profit-Taker, Tridolon, Arbitrations
- Market Analytics and Missions/Enemies views
- the custom theme builder and custom CSS
- dashboard layout editing and pop-out widgets
- auto-update, Mastery "Planned"/"Codex"/"Incarnon" tabs

## Tests

```sh
cargo test --workspace
cargo run -p fluffyframe-core --example smoke   # live check against the real endpoints
```

# Credits

- [WFHelper](https://github.com/WFHelper/WFHelper) - used as a starting base for the project
- [calamity-inc](https://github.com/calamity-inc) - most unofficial data. Invigorations forecast was ported from their [oracle](https://github.com/calamity-inc/wf.browse.oracle/tree/senpai)
- [warframe.market](https://warframe.market) - direct market integration. Everybody uses, everybody owes them
- [DE's Public export](https://content.warframe.com/PublicExport/index_en.txt.lzma), [world state](https://api.warframe.com/cdn/worldState.php) - official info
- all the other guys who are included in OG project and known in community

# Disclaimer

Project is not affiliated with Digital Extremes. Warframe and all its assets are property of Digital Extremes Ltd.
