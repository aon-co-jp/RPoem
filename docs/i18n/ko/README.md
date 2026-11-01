# RPoem — 개요

- RPoem은 Rust로 만든 GraphQL Federation 플랫폼입니다(WunderGraph Cosmo 호환).
- REST의 /api/는 2026-11-01에 전면 폐지했습니다. 관리 작업은 gRPC(같은 정의를 gRPC-web·Connect로도 수신)로 하며 하나의 포트에서 받습니다.
- GraphQL은 Federation 조회 전용이며, 운영에서는 네 가지 방어(영구 쿼리 강제, 깊이/복잡도 제한, 인트로스펙션 비활성화, 배치 거부)를 켭니다.
- gRPC는 TLS 1.3 필수, 필요 시 mTLS. 브라우저 화면은 WASM(382KB)이며 RPC Inspector와 개발자 도구 확장(apps/devtools-extension)을 갖춥니다.
- 프로토콜 공통 부품은 open-runo-connect-core / server / client입니다(기존 저장소 gRPC-web-Connect를 RPoem에 통합).

> 한국어 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
