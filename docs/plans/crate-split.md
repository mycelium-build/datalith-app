# Plan de découpage en crates

Migrer par étapes en conservant les points d’entrée desktop, iOS et Android. Chaque étape doit pouvoir être compilée et vérifiée indépendamment. [Décision d’architecture](../adr/0004-split-native-applications-into-crates.md)

## Renovate dans la PR de découpage

Mettre à jour la [configuration Renovate](../../renovate.json) dans la première PR, avec les nouveaux manifestes Cargo. Vérifier par une simulation de détection que Renovate trouve les dépendances déclarées à la racine du workspace, dans `crates/*/Cargo.toml` et dans `apps/desktop/Cargo.toml`. Vérifier qu’une mise à jour Cargo continue de mettre à jour le `Cargo.lock` du workspace et de régénérer les notices de licences via le [script post-upgrade](../../scripts/renovate-post-upgrade.sh). La PR mobile vérifiera à son tour la détection des manifestes qu’elle ajoute.

## 1. Extraire les formats et le coffre

Déplacer les formats, le parsing et la validation des expressions dans `datalith-core`. Extraire ensuite les opérations sur fichiers, le catalogue et la recherche dans `datalith-vault`, qui dépend de `datalith-core`. Faire consommer les crates extraites au [benchmark du catalogue](../../crates/datalith-vault/benches/catalog.rs).

> [!WARNING]
> Les Bases et le catalogue partagent un contrat. Un fichier `.base` peut appeler uniquement les fonctions et méthodes autorisées par le [validateur d’expressions](../../crates/datalith-core/src/expr/validate.rs). Leurs noms et signatures relèvent de `datalith-core` ; leur traduction en requêtes relève du [compilateur du catalogue](../../crates/datalith-vault/src/catalog/database/query/compiler.rs) dans `datalith-vault`. Toute évolution de cette liste doit être vérifiée des deux côtés.

Tester cette frontière avec un petit coffre contenant une note qui pointe vers `Welcome` et une autre sans ce lien. Valider le filtre `file.hasLink("Welcome")`, l’exécuter et vérifier que seule la première note correspond. Vérifier aussi que le validateur rejette une fonction inconnue et un appel à `file.hasLink()` sans argument.

## 2. Isoler le serveur du Clipper

Extraire `datalith-server` et raccorder son démarrage et ses réglages au desktop. Vérifier la sauvegarde d’une note via le Clipper et l’absence du serveur dans l’arbre de dépendances Android et iOS.

## 3. Séparer les interfaces

Placer les éditeurs et visionneuses communs dans `datalith-ui`, puis raccorder la fenêtre et les menus à `datalith-desktop`, la navigation tactile à `datalith-mobile`. Sur chaque cible, modifier une note, passer en lecture et vérifier que le texte affiché correspond au tampon d’édition. Relancer hors ligne et retrouver la modification par la recherche.

### Préserver le registre des types de fichiers

Aujourd’hui, `default_registry()` associe chaque extension à un `FileTypeConfig` qui déclare ses capacités (recherche textuelle, wikiliens, frontmatter YAML), son icône, son mode initial, ses fabriques optionnelles d’éditeur et de visionneuse, et son adaptateur de rechargement. `registered_file_types()` en extrait les seules capacités dont le catalogue et l’index ont besoin. [Registre et fabriques](../../crates/datalith-ui/src/registry.rs) · [Types utilisés par le coffre](../../crates/datalith-core/src/file_types.rs)

Lors du découpage, faire porter la correspondance extension → type et les capacités par `datalith-core`, afin que `datalith-vault` puisse les utiliser directement. Dans `datalith-ui`, associer les fabriques, l’icône, le mode initial et le rechargement au type résolu. Garder une seule liste d’extensions, y compris les alias d’images. À l’ouverture, préserver l’ordre de `create_handler()` : il crée l’éditeur, lui transmet le catalogue si disponible, puis fournit cet éditeur à la fabrique de visionneuse.

Vérifier qu’une note Markdown passe en lecture sur le même tampon, qu’une Base reçoit aussi les données du catalogue, qu’une image s’ouvre en lecture seule côté interface et qu’un fichier `.todotxt` dispose de son éditeur. Une extension inconnue conserve sa configuration de repli et reste hors des types suivis par le catalogue.

Pour un futur client web, le point de réutilisation est la construction d’une vue pour un type donné. Les fabriques actuelles acceptent un chemin et la vue de Base reçoit un `VaultCatalog` local. Lors du premier port d’une vue, lui fournir le contenu ou les résultats de requête depuis l’application hôte permettra d’évaluer la réutilisation de la fabrique GPUI avec un coffre hébergé.
