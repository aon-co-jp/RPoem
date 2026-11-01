# RPoem — regole di sviluppo (per l'IA)

- L'unica fonte della definizione dell'API amministrativa è crates/open-runo-proto/proto/.../admin.proto.
- Si formatta con cargo fmt -p <crate>, mai con --all (formatterebbe anche i repository vicini).
- Non si fa commit delle cartelle non tracciate RPoem/ e target-crossui/. I segreti (chiavi, token) non devono finire nel repository né nei log.
- Prima di dichiarare il lavoro finito, verificare visualizzazione e funzionamento in un browser reale (non perdere uno schermo bianco).
- Sul VPS solo git pull; riavvio dei servizi o apertura di porte solo su istruzione esplicita dell'utente.

> Italiano · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の CLAUDE.md (ja/en).
