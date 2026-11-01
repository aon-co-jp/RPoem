# RPoem — обзор

- RPoem — платформа GraphQL Federation на Rust (совместима с WunderGraph Cosmo).
- REST-эндпоинты /api/ полностью удалены 2026-11-01. Административные операции выполняются через gRPC (то же описание принимается и как gRPC-web, и как Connect) на одном порту.
- GraphQL используется только для запросов Federation; в продакшене включены четыре защиты (обязательные сохранённые запросы, лимиты глубины и сложности, отключённая интроспекция, запрет пакетных запросов).
- gRPC требует TLS 1.3, при необходимости — mTLS. Браузерный интерфейс — WASM (382 КБ) с RPC Inspector и расширением для DevTools (apps/devtools-extension).
- Общие компоненты протоколов — open-runo-connect-core / server / client (прежний репозиторий gRPC-web-Connect влит в RPoem).

> Русский · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
