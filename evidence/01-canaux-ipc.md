# 01 — Canaux IPC du main process Obsidian

Extraction statique REA (`analyze_javascript_application` sur `main.js` ; vue `javascript-facts` kind `ipc-channel`/`ipc-handler`).

- **SEEN** — 90 observations : 45 handlers + 45 canaux ; 45 canaux uniques.
- **SEEN** — 100 % côté main process (`side: main`). 2 observations `handle` (`register-cli`), 88 `listen`.
- **SEEN** — Aucune transmission renderer appariée, aucun canal dynamique.
- **UNKNOWN** — Libellés/contrats de payload, ordre d'envoi, et comportement runtime non élucidés (analyse inerte).
- **INFERRED** — Sans preload/contextBridge ni transmission renderer statique, les appels renderer→main passent par un chemin non appariable statiquement (probable `nodeIntegration`), non prouvé.

## Canaux (45, triés)

```
adblock-frequency
adblock-lists
check-update
cli
context-menu
copy-asar
create-browser-session
desktop-dir
disable-gpu
disable-update
documents-dir
file-url
frame
get-default-vault-path
get-documents-path
get-icon
get-sandbox-vault-path
help
insider-build
is-closing
is-quitting
open-url
policy
print-to-pdf
register-cli
relaunch
render-menu
request-url
resources
sandbox
set-icon
set-language
set-menu
starter
terms
trash
update
update-menu-items
vault
vault-list
vault-message
vault-move
vault-open
vault-remove
version
```

Nota : liste exhaustive de l'analyse statique ; les canaux dynamiques éventuels (names construits à l'exécution) ne sont pas captés (limitation REA déclarée).
