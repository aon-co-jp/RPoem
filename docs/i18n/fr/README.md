# RPoem — aperçu

- RPoem est une plateforme GraphQL Federation écrite en Rust (compatible avec WunderGraph Cosmo).
- Les points d'accès REST /api/ ont été entièrement supprimés le 2026-11-01. Les opérations d'administration passent par gRPC (la même définition est aussi acceptée en gRPC-web et en Connect) sur un seul port.
- GraphQL ne sert plus qu'aux requêtes de Federation ; en production, quatre protections sont actives (requêtes persistées obligatoires, limites de profondeur et de complexité, introspection désactivée, requêtes par lots refusées).
- gRPC exige TLS 1.3, avec mTLS en option. L'interface dans le navigateur est en WASM (382 Ko) avec RPC Inspector et une extension pour les outils de développement (apps/devtools-extension).
- Les briques communes des protocoles sont open-runo-connect-core / server / client (l'ancien dépôt gRPC-web-Connect a été intégré à RPoem).

> Français · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
