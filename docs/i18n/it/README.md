# RPoem — panoramica

- RPoem è una piattaforma GraphQL Federation scritta in Rust (compatibile con WunderGraph Cosmo).
- Gli endpoint REST /api/ sono stati eliminati del tutto il 2026-11-01. Le operazioni amministrative usano gRPC (la stessa definizione è accettata anche come gRPC-web e Connect) su un'unica porta.
- GraphQL serve solo per le query di Federation; in produzione sono attive quattro protezioni (query persistite obbligatorie, limiti di profondità e complessità, introspezione disattivata, richieste batch rifiutate).
- gRPC richiede TLS 1.3, con mTLS opzionale. L'interfaccia nel browser è WASM (382 KB) con RPC Inspector e un'estensione per DevTools (apps/devtools-extension).
- I componenti comuni dei protocolli sono open-runo-connect-core / server / client (il vecchio repository gRPC-web-Connect è stato integrato in RPoem).

> Italiano · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
