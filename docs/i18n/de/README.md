# RPoem — Überblick

- RPoem ist eine in Rust geschriebene GraphQL-Federation-Plattform (kompatibel mit WunderGraph Cosmo).
- Die REST-Endpunkte unter /api/ wurden am 2026-11-01 vollständig entfernt. Administrative Aufrufe laufen über gRPC (dieselbe Definition wird auch als gRPC-web und Connect akzeptiert) auf einem einzigen Port.
- GraphQL dient nur noch Federation-Abfragen; in Produktion sind vier Schutzmaßnahmen aktiv (persistierte Abfragen erzwungen, Tiefen- und Komplexitätslimits, Introspektion aus, Batch-Anfragen abgelehnt).
- gRPC verlangt TLS 1.3, optional mTLS. Die Browser-Oberfläche ist WASM (382 KB) mit RPC Inspector und einer DevTools-Erweiterung (apps/devtools-extension).
- Die gemeinsamen Protokoll-Bausteine sind open-runo-connect-core / server / client (das frühere Repository gRPC-web-Connect wurde in RPoem integriert).

> Deutsch · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
