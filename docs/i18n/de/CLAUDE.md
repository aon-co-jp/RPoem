# RPoem — Entwicklungsregeln (für KI)

- Einzige maßgebliche Definition der Admin-API ist crates/open-runo-proto/proto/.../admin.proto.
- Formatiert wird mit cargo fmt -p <Crate>, nie mit --all (sonst werden Nachbar-Repositories mitformatiert).
- Nicht verfolgte Ordner RPoem/ und target-crossui/ werden nicht committet. Geheimnisse (Schlüssel, Token) gehören weder ins Repository noch in Logs.
- Vor der Fertigmeldung Darstellung und Bedienung im echten Browser prüfen (weißen Bildschirm nicht übersehen).
- Auf dem VPS nur git pull; Dienste neu starten oder Ports öffnen nur auf ausdrückliche Anweisung des Nutzers.

> Deutsch · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の CLAUDE.md (ja/en).
