# RPoem — règles de développement (pour l'IA)

- La seule source de référence de l'API d'administration est crates/open-runo-proto/proto/.../admin.proto.
- Le formatage se fait avec cargo fmt -p <crate>, jamais avec --all (cela formaterait aussi les dépôts voisins).
- Ne pas valider les dossiers non suivis RPoem/ et target-crossui/. Les secrets (clés, jetons) ne doivent apparaître ni dans le dépôt ni dans les journaux.
- Avant d'annoncer que c'est terminé, vérifier l'affichage et le fonctionnement dans un vrai navigateur (ne pas manquer un écran blanc).
- Sur le VPS, uniquement git pull ; redémarrer un service ou ouvrir un port seulement sur instruction explicite de l'utilisateur.

> Français · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の CLAUDE.md (ja/en).
