# RPoem — passation

- Fait : administration via gRPC, hybride gRPC-web et Connect, traitement des protocoles remplacé par des crates communs, WASM réduit (751 Ko → 382 Ko), extension pour les outils de développement, analyse RustSec (version-control).
- À faire : prise en charge de HTTP/2 dans open-web-server (le gRPC pur exige des réponses en flux avec trailers).
- À faire : transport en flux pour connect-rust (l'amont est encore avant la 1.0 ; le crate client maison le remplace pour l'instant).
- En attente : la branche rest-to-graphql d'open-web-server est poussée mais non fusionnée (l'appelé doit être la nouvelle passerelle).
- Reprendre à la section « changement de cap (gRPC) » au début de CLAUDE.md et à cette passation.

> Français · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の PORTING.md (ja/en).
