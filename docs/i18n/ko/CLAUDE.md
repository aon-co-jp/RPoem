# RPoem — 개발 규칙(AI용)

- 관리 API 정의의 원본은 crates/open-runo-proto/proto/.../admin.proto 하나뿐입니다.
- 포맷은 cargo fmt -p <크레이트>를 쓰고 --all은 쓰지 않습니다(형제 저장소까지 포맷되기 때문).
- 추적되지 않는 RPoem/ 과 target-crossui/ 는 커밋하지 않습니다. 비밀(키·토큰)은 저장소나 로그에 남기지 않습니다.
- 완료를 보고하기 전에 실제 브라우저에서 표시와 동작을 확인합니다(백화면을 놓치지 않기).
- VPS에는 git pull만 합니다. 서비스 재시작이나 포트 공개는 사용자의 명시적 지시가 있을 때만 합니다.

> 한국어 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の CLAUDE.md (ja/en).
