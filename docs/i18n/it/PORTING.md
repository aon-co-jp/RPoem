# RPoem — passaggio di consegne

- Fatto: amministrazione tramite gRPC, ibrido gRPC-web e Connect, gestione dei protocolli sostituita da crate comuni, WASM ridotto (751 KB → 382 KB), estensione DevTools, scansione RustSec (version-control).
- Da fare: supporto HTTP/2 in open-web-server (per il gRPC puro serve una risposta in streaming con trailer).
- Da fare: trasporto in streaming per connect-rust (upstream ancora prima della 1.0; per ora si usa il crate client interno).
- In sospeso: il branch rest-to-graphql di open-web-server è stato pubblicato ma non unito (la controparte deve essere il nuovo gateway).
- Si riparte dalla sezione «cambio di rotta (gRPC)» all'inizio di CLAUDE.md e da queste consegne.

> Italiano · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の PORTING.md (ja/en).
