# README — Index de l'evidence

Index des fichiers d'evidence du projet (artefact Obsidian, client clean-room Rust + iced).

| Fichier | Type | Description courte | Statut |
| --- | --- | --- | --- |
| `00-architecture-overview.md` | Markdown — doc d'architecture | Vue d'ensemble de l'architecture de l'artefact Obsidian : constats d'inventaire REA labellisés SEEN / INFERRED / UNKNOWN (méthode, architecture globale, modules, chargement, plugins, points d'entrée, probes REA). | Existant |
| `rea-bundle.json` | JSON — bundle d'evidence REA | Bundle canonique d'evidence : 5 enregistrements `analysis_profile` (`inspect_analysis_view` ×3, `inspect_artifact`, `inventory_artifact`), 1 artefact (`obsidian.asar`, SHA-256 `a52a7daf…`, format asar), 2 providers. | Existant |
| Synthèses d'analyse (à venir) | Markdown / JSON — à définir | Résultats des analyses JavaScript (renderer `app.js`, bundles `lib/`, analyses complémentaires) et synthèses dérivées, produit par les analyses REA en cours. | À venir |
