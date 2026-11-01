# RPoem — 인수인계

- 완료: 관리 작업의 gRPC화, gRPC-web·Connect 하이브리드, 통신 처리를 공통 크레이트로 교체, WASM 축소(751KB→382KB), 개발자 도구 확장, RustSec 검사(version-control).
- 미완: open-web-server의 HTTP/2 지원(순수 gRPC는 트레일러가 있는 스트리밍 응답으로의 재작성이 필요).
- 미완: connect-rust의 스트리밍 전송(업스트림이 1.0 이전이라 자체 client 크레이트로 대체 중).
- 보류: open-web-server의 rest-to-graphql 브랜치는 push했지만 병합하지 않음(호출 대상이 새 게이트웨이여야 함).
- 재개는 CLAUDE.md 맨 앞의 방침 전환(gRPC) 절과 이 인수인계에서 시작하세요.

> 한국어 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の PORTING.md (ja/en).
