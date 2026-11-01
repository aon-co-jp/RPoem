# RPoem — огляд

- RPoem — платформа GraphQL Federation на Rust (сумісна з WunderGraph Cosmo).
- REST-ендпоїнти /api/ повністю видалено 2026-11-01. Адміністративні операції виконуються через gRPC (той самий опис приймається також як gRPC-web і Connect) на одному порту.
- GraphQL використовується лише для запитів Federation; у продакшені ввімкнено чотири захисти (обов'язкові збережені запити, обмеження глибини та складності, вимкнена інтроспекція, заборона пакетних запитів).
- gRPC вимагає TLS 1.3, за потреби — mTLS. Браузерний інтерфейс — WASM (382 КБ) з RPC Inspector і розширенням для DevTools (apps/devtools-extension).
- Спільні компоненти протоколів — open-runo-connect-core / server / client (колишній репозиторій gRPC-web-Connect злито в RPoem).

> Українська · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
