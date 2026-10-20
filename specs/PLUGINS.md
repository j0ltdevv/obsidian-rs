# Objectif

Documenter le système de plugins : une API minimale (`api::Plugin`)
définissant les points d'extension, un registre (`registry`) gérant
l'enregistrement, le chargement, la notification et le déchargement, et les
descripteurs exposés à l'UI.

## Comportements

### 1. Identité et enregistrement

1. Chaque plugin possède un `id` stable et unique, servant de clé
   d'identification dans le registre. — INFERRED
2. L'enregistrement d'un plugin dont l'`id` est déjà enregistré échoue avec une
   erreur de doublon ; le plugin existant n'est ni remplacé ni modifié. —
   INFERRED
3. Le `name` est un libellé humain affichable, distinct de l'`id`. — INFERRED
4. Le registre est un ajout : l'enregistrement d'un plugin avec un `id` inédit
   réussit et rend le plugin visible du registre. — INFERRED

### 2. Cycle de vie

5. `on_load` est appelé au chargement du plugin (à l'activation), une seule fois
   par plugin chargé. — INFERRED
6. `on_change` est notifié à chaque mutation du vault (création, modification ou
   suppression de note, par exemple) pour tout plugin chargé. — INFERRED
7. `on_unload` est appelé à l'arrêt (déchargement) de chaque plugin chargé. —
   INFERRED

### 3. Gestion des erreurs

8. Les erreurs levées par un point d'extension se propagent à l'appelant ; elles
   ne sont ni avalées ni transformées en simple journal. — INFERRED
9. Un échec de chargement ne laisse pas de plugin mi-chargé : le plugin est
   refusé et le registre reste dans un état cohérent, sans interruption partielle
   des autres plugins. — INFERRED
10. Un échec de déchargement ne laisse pas de plugin mi-déchargé : le registre
    reste cohérent et l'erreur est propagée. — INFERRED

### 4. Descripteurs pour l'UI

11. Le registre expose, pour chaque plugin enregistré, un descripteur avec au
    moins `id`, `name` et `enabled`. — INFERRED
12. `enabled` reflète l'état actif du plugin (chargé/déchargé) et peut être
    utilisé par l'UI pour afficher l'état. — INFERRED

## Limites et hors périmètre

- Pas d'exécution de code externe : les plugins sont compilés dans le binaire
  (Rust) ; aucun script ni code n'est chargé depuis le disque ou le réseau au
  runtime. — INFERRED
- Pas d'API de rendu : un plugin ne peut pas fournir de vues, widgets ni
  extensions d'interface ; les descripteurs ne servent qu'à l'affichage. —
  INFERRED
- Pas de hot-reload : l'ajout, le retrait ou la modification d'un plugin exige
  une recompilation et un redémarrage ; le registre est construit au démarrage.
  — INFERRED

## Label

Tous les comportements ci-dessus sont étiquetés **INFERRED** : spécification de
conception du système de plugins (API minimale + registre + descripteurs UI),
sans observation REA.
