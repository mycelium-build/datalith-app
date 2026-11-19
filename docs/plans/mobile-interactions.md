# De chaque vue desktop à son usage tactile

Ce plan pour la PR #89 part de ce que l’on vient faire dans chaque vue desktop. Les parcours tactiles sont des propositions à essayer. Les observations du prototype sont consignées à la fin. Un changement de largeur peut modifier la disposition pendant que l’utilisateur lit, écrit ou déplace une carte.

## Coffres, fichiers et documents ouverts

| Vue desktop et tâche | Parcours tactile proposé | Essai de la vue |
| --- | --- | --- |
| **Lancement et choix du coffre.** Retrouver ses fichiers ou découvrir le coffre de documentation. | Depuis l’accueil, toucher le nom du coffre pour retrouver le dernier dossier. Pour un coffre externe, passer par le sélecteur système si la prise en charge de l’accès persistant est retenue. Identifier la documentation en lecture seule. | Relancer depuis un coffre vide, puis depuis un coffre déjà parcouru. Si l’accès a été révoqué, permettre de le redonner depuis le coffre concerné. |
| **Arborescence.** Explorer les dossiers et agir sur un fichier. | Sur téléphone, toucher une ligne ouvre son dossier ou son document. En grand format, garder la liste visible et la sélection marquée. Le menu de la ligne présente les actions autorisées sur ce fichier. Pour déplacer, choisir un dossier de destination. Après suppression, afficher Annuler dans la bannière basse. | Supprimer puis restaurer une note dans un sous-dossier. Vérifier son chemin, la position dans la liste et l’absence d’actions d’écriture sur la documentation. |
| **Palette de recherche.** Retrouver une note par nom ou contenu. | La loupe ouvre le champ et les résultats. Afficher la portée effective. Toucher un résultat ouvre le document. Retour retrouve la requête, le résultat et la position. Réutiliser le catalogue desktop pour une éventuelle recherche dans tout le coffre. | Depuis la racine, chercher « Library ». Un filtre du dossier doit indiquer sa limite. La recherche dans tout le coffre doit retrouver la Base du sous-dossier. |
| **Onglets et changement rapide.** Reprendre un autre document en cours. | L’historique sert à revenir au point de départ. Pour passer directement entre documents ouverts, essayer une liste des titres avec leur dossier si deux noms coïncident. Chaque document garde sa session. | Saisir dans A, consulter B, reprendre A et retrouver son tampon et sa sélection. Annuler à mi-course le geste Retour depuis B doit laisser B affiché. |

Le sélecteur et le contenu utilisent le thème GPUI de Datalith. Sur un téléphone classique, le système anime le passage liste → document. Sur le Duo fermé, Retour occupe le rail de droite et le contenu utilise toute la hauteur, sans en-tête ajouté. Sur le Duo ouvert et sur iPad, afficher liste et détail ensemble dès que les deux restent lisibles à la taille de texte choisie. Sur Android, le geste Retour et la commande visible agissent sur la même destination. [Arbre desktop](../../apps/desktop/src/ui/sidebar/file_tree.rs) · [Palette](../../apps/desktop/src/ui/palette.rs) · [Documents ouverts](../../apps/desktop/src/ui/tabs/workspace.rs) · [Guide Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo) · [Disposition Android](https://developer.android.com/develop/adaptive-apps/guides/list-detail)

## Documents

| Vue desktop et tâche | Parcours tactile proposé | Essai de la vue |
| --- | --- | --- |
| **Lecture Markdown.** Lire le texte rendu, ses propriétés et ses wikiliens. | Garder le rendu des blocs et la typographie Datalith. Glisser pour lire, toucher un lien pour le suivre, revenir au paragraphe d’origine. L’icône crayon ouvre la source si le document est éditable. | Depuis une note longue, suivre un wikilien, revenir, puis passer en édition. Vérifier le passage retrouvé, le nom du document et son contenu. |
| **Édition Markdown.** Écrire et compléter un wikilien. | Le toucher place le curseur dans l’éditeur partagé. Garder le passage actif et les suggestions visibles au-dessus du clavier. Toucher une suggestion insère le lien. La lecture reprend le même tampon. | Composer un accent, insérer un emoji et un wikilien, puis déplier le Duo pendant la saisie. Vérifier la sélection, l’aperçu et le texte enregistré. |
| **Texte brut et fichiers sans vue spécialisée.** Lire ou corriger le contenu. | Utiliser le traitement du registre et le même éditeur texte. Autoriser la sélection et la copie en lecture seule. | Ouvrir un fichier texte d’extension inconnue, modifier puis rouvrir. Rejouer sur un fichier protégé et vérifier qu’il reste intact. |
| **todo.txt.** Cocher une tâche, en saisir une autre, filtrer ou trier. | Distinguer la case à cocher de la zone de texte qui ouvre l’édition. Rendre priorité, filtre et tri accessibles au toucher. Garder la tâche en cours visible au-dessus du clavier. | Avec un filtre actif, terminer une tâche qui disparaît des résultats. Vérifier dans le fichier que seule cette tâche a changé, puis ajouter une tâche. |
| **Image.** Examiner une pièce jointe. | Ouvrir l’image entière sur le fond du thème. Sur un grand écran, ajuster au panneau de lecture. Pour examiner un détail, essayer le zoom et une commande de retour au cadrage initial. | Ouvrir une image en portrait, pivoter, revenir au dossier et retrouver sa ligne. Vérifier que la visionneuse propose uniquement des actions de consultation. |

Le registre construit éditeurs et visionneuses. Le lecteur Markdown et l’aperçu de Base observent chacun le tampon de leur éditeur. Conserver cette relation lorsque le clavier, le mode ou le nombre de panneaux change. [Registre](../../crates/datalith-ui/src/registry.rs) · [Lecture Markdown](../../crates/datalith-ui/src/viewers/markdown/mod.rs) · [Tâches](../../crates/datalith-ui/src/editors/todo_txt/render.rs) · [Images](../../crates/datalith-ui/src/viewers/image.rs)

## Bases

Le nom de la vue active sert de sélecteur. Le toucher affiche **les vues définies dans ce fichier**, dans leur ordre, avec la sélection actuelle. Ce nom identifie du contenu et doit rester lisible, y compris sur le Duo où le sélecteur peut s’ouvrir depuis le rail. Chaque vue garde son défilement et sa sélection pendant la session. Le catalogue, les filtres et les regroupements du fichier alimentent toutes les présentations. [Visionneuse](../../crates/datalith-ui/src/viewers/base/mod.rs) · [Library.base](../vault/examples/bases/Library.base) · [Overview.base](../vault/Overview.base)

| Vue desktop et tâche | Parcours tactile proposé | Essai de la vue |
| --- | --- | --- |
| **Liste.** Parcourir rapidement des entrées déjà triées ou groupées. | Faire défiler une seule liste verticale. Garder le nom et les propriétés configurées lisibles sur plusieurs lignes au besoin. Toucher le nom ouvre la note. | Dans *Recently touched*, ouvrir une entrée puis revenir. Retrouver la ligne au même endroit et vérifier l’ordre des résultats. |
| **Table.** Comparer la même propriété sur plusieurs notes. | Garder les valeurs alignées et l’identité de la ligne visible pendant le défilement horizontal. Toucher une ligne ouvre ses propriétés en fiche, avec une action pour ouvrir la note. Sur grand écran, utiliser la largeur pour montrer davantage de colonnes. | Dans *Library by status*, comparer deux livres sur une colonne éloignée. Passer à *Cross references* puis revenir. Chaque table retrouve son propre décalage. |
| **Cartes.** Reconnaître un livre par sa couverture puis l’ouvrir. | Conserver couverture, titre et propriétés de la carte GPUI. Adapter le nombre de colonnes à la largeur et au texte agrandi. Toute la carte ouvre le document. | Dans *Covers*, ouvrir un livre en touchant son image puis revenir au même rang. Tester aussi un titre long et une couverture absente. |
| **Kanban.** Lire les livres d’une colonne ou changer leur classement. | En compact, afficher une colonne et son nom comme sélecteur donnant accès à toutes les colonnes. En large, en montrer plusieurs. Toucher une carte ouvre la note. Son menu permet de choisir une destination lorsque la règle desktop autorise le déplacement. | Dans *Reading board*, atteindre la colonne *reading* et ouvrir une carte. Dans une copie éditable, déplacer un livre puis vérifier son classement après réouverture. En lecture seule, conserver la consultation. |
| **Source YAML.** Modifier la définition et examiner son effet. | Lorsque l’édition est offerte, ouvrir l’éditeur de Base partagé et revenir à la vue nommée si elle existe encore. Rattacher les erreurs de définition à cette Base. La disponibilité sur téléphone reste à arbitrer. | Dans une copie, renommer une vue en YAML et vérifier le sélecteur. Introduire puis corriger une erreur avec le clavier affiché. |

La commande tactile de déplacement du kanban doit partager les conditions déjà utilisées pour le glisser desktop, notamment pour les groupes calculés et les fichiers protégés. [Liste](../../crates/datalith-ui/src/viewers/base/list.rs) · [Table](../../crates/datalith-ui/src/viewers/base/table.rs) · [Cartes](../../crates/datalith-ui/src/viewers/base/cards/mod.rs) · [Kanban](../../crates/datalith-ui/src/viewers/base/kanban.rs) · [Source YAML](../../crates/datalith-ui/src/editors/base.rs)

### Graphe — identifier une note à partir de ses liens

Le canevas écoute la souris et la molette. Le même déplacement fait bouger un nœud ou déplace la caméra selon l’endroit où le doigt tombe. Quatre unités de déplacement suffisent pour transformer un appui en glissement. La cible est le rayon dessiné dans le repère du graphe : dézoomer la rend de plus en plus petite. Le cadrage initial plafonne le zoom à 1,5 ; le rendu attend 2,5 avant d’afficher les noms sans survol. Un graphe fraîchement cadré démarre donc sans libellés persistants. Relâcher un nœud ouvre directement son document. Un utilisateur au doigt doit donc identifier une note avant de pouvoir lire son nom. [Interactions](../../crates/datalith-ui/src/viewers/base/graph/mod.rs) · [Cibles et libellés](../../crates/datalith-ui/src/viewers/base/graph/model.rs)

### Contrat tactile à prototyper

| Intention | Interaction proposée | Résultat vérifiable |
| --- | --- | --- |
| Explorer | Glisser à un doigt pour déplacer la caméra, y compris depuis un nœud. Pour la première livraison, zoomer avec +/− et recadrer avec Ajuster. Ajouter le pincement autour du milieu des doigts une fois le backend validé. | Le point du graphe sous les doigts reste stable. Un panoramique terminé sur une note ne l’ouvre pas. Le bord réservé au Retour système reste disponible. |
| Identifier | Toucher un nœud pour le sélectionner. Stabiliser les positions pendant la manipulation. | Son nom et ses connexions deviennent lisibles. Agrandir la zone tactile dans le repère de l’écran. Si plusieurs nœuds sont indissociables au toucher, proposer leurs noms. |
| Ouvrir et revenir | Une fiche présente le nom, les liens et l’action Ouvrir. Sur un petit écran, la fiche apparaît en bas, au-dessus de la zone système. | Le retour depuis la note restaure la vue de Base, le nœud et la caméra. Fermer la fiche laisse le graphe visible. |
| Explorer sans pincer | Offrir Ajuster à l’écran et un accès par noms aux notes et relations du graphe. | Une personne peut retrouver et ouvrir la même note avec un seul doigt ou un lecteur d’écran. Le dessin ne constitue pas son seul point d’entrée. |

Les gestes proposés suivent les conventions iOS ; l’alternative à une manipulation multipoint reprend un principe d’accessibilité également documenté par le W3C pour le Web. Sur Android, préserver les gestes système des deux bords. [Gestes Apple](https://developer.apple.com/design/human-interface-guidelines/gestures) · [Alternatives aux gestes multipoints](https://www.w3.org/WAI/WCAG22/Understanding/pointer-gestures.html) · [Navigation gestuelle Android](https://developer.android.com/develop/ui/views/touch-and-input/gestures/gesturenav)

**Option pour les graphes denses.** Permettre d’isoler les voisins de la note sélectionnée, puis de revenir à la carte entière. Da Lozzo, Di Battista et Ingrassia ont implémenté une exploration de graphe par voisinage sur smartphone. Pour Datalith, l’essai doit déterminer si ce mode aide à retrouver une note sans faire perdre le contexte de la Base. [Étude originale, *Drawing Graphs on a Smartphone*](https://jgaa.info/index.php/jgaa/article/view/paper252)

Sur grand écran, essayer la fiche de sélection à côté du graphe. Dans *Overview.base → Wiki*, ouvrir une note depuis sa fiche puis revenir à la même caméra et à la sélection. Ouvrir ensuite le même fichier depuis le coffre et un wikilien pour vérifier la résolution des chemins.

## Réglages, thèmes et informations

| Vue desktop | Transposition et vérification |
| --- | --- |
| **Général et apparence.** | Présenter les préférences pertinentes sous forme de lignes tactiles. Essayer une taille de texte supérieure puis rouvrir la note et les réglages. Vérifier que les libellés restent lisibles sur les deux surfaces. |
| **Choix du thème et aperçu.** | Toucher un thème pour examiner son rendu sur la liste et le document. Conserver les couleurs, polices et tokens du thème Datalith. La création et l’import/export de thèmes relèvent de l’atelier desktop, dont le port est différé dans la proposition de périmètre. |
| **À propos, licences et documentation.** | Regrouper leurs entrées dans les réglages. Ouvrir le texte d’une licence ou une note de documentation, le parcourir en texte agrandi puis revenir à l’entrée d’origine. |
| **Raccourcis, serveur et mises à jour.** | Les commandes tactiles appellent les actions partagées. Les raccourcis desktop restent dans l’application desktop. Le serveur Clipper et la gestion des mises à jour desktop y gardent leurs écrans. |

[Réglages desktop](../../apps/desktop/src/ui/settings/mod.rs) · [Atelier de thèmes](../../apps/desktop/src/ui/themes/editor/mod.rs) · [Licences](../../apps/desktop/src/ui/licenses.rs) · [Périmètre mobile et arbitrages](mobile-scope.md)

## Responsabilités communes aux vues


- **`datalith-core` et `datalith-vault`** conservent les définitions de vues, les chemins, les relations, la recherche et les opérations sur les fichiers.
- **`datalith-ui`** conserve les tampons, visionneuses, modèles de Base, caméra et moteur du graphe. Ajouter une interaction tactile qui exploite ces modèles. GPUI 0.3.12 expose `on_pinch`, mais son reconnaisseur tactile ignore les contacts supplémentaires. Son glissé à un doigt peut produire un événement de défilement que le graphe interprète comme un zoom. Prototyper la prise en charge du déplacement tactile via `TouchDragEvent` et vérifier tap, glissé et annulation sur chaque backend avant de toucher aux dépendances. [Reconnaisseur de GPUI 0.3.12](https://docs.rs/crate/gpui-pre/0.3.12/source/src/gestures.rs#527) La sélection persistante doit être distincte du survol desktop.
- **`apps/mobile`** possède l’historique de navigation, les présentations temporaires et la restauration. La couche GPUI rend le contenu et expose des actions sémantiques pour les lignes et les nœuds. La coque native relaie les contrôles système, le clavier, les zones réservées et le cycle du geste Retour. Éviter deux historiques qui peuvent diverger.
- **Accessibilité** : vérifier avec VoiceOver et TalkBack que les éléments annoncent nom, rôle, sélection et action. Tester des cibles de 44 points sur iOS et de 48 dp sur Android, texte agrandi, réduction des animations et alternative à chaque geste complexe. [Apple](https://developer.apple.com/design/human-interface-guidelines/accessibility) · [Android](https://developer.android.com/guide/topics/ui/accessibility/apps)

Chaque parcours ci-dessus doit pouvoir être rejoué sur téléphone classique, Duo et iPad, puis sur Android avec le Retour système. Vérifier les transitions avec réduction des animations, l’annulation du geste Retour et la restauration après changement de largeur. Pour une erreur d’ouverture, l’afficher sur la vue à l’origine de la demande et permettre de la fermer sur place.

## Tour de Claude sur iPhone 17, iOS 27.2

| Parcours effectivement joué | Observation | Conséquence pour la refonte |
| --- | --- | --- |
| `Overview.base → Wiki`, glissé vertical, horizontal, pincement | Le glissé vertical change le zoom. Le glissé horizontal et le pincement n’ont pas déplacé la carte. Après plusieurs gestes, Claude a dû quitter et rouvrir la Base pour retrouver les nœuds. | Priorité au panoramique et à Ajuster, accessibles immédiatement. |
| Toucher un nœud → ouvrir sa note | Claude a obtenu « Cannot open …/formats/bases/Overview.md ». Le même fichier s’ouvrait depuis le coffre. Le bandeau d’erreur a persisté jusque dans l’éditeur. | Une seule commande d’ouverture, avec des entrées distinctes pour chemin du coffre et lien relatif au document. Tester le même fichier depuis trois origines. Rattacher l’erreur à la destination, avec fermeture accessible. |
| Recherche depuis la racine | « Welc » retrouve Welcome.md. « Library » donne zéro résultat alors que la Base existe dans un sous-dossier. Retour conserve la requête. | Afficher la portée effective et brancher la recherche du coffre sur le catalogue. |
| `Library.base`, table, Covers, Reading board, Cross references | Le tableau groupé défile horizontalement. Le changement de vue fonctionne. Cross references hérite du décalage de la table précédente. Sur le board, Claude a défilé dans une colonne, mais n’a pu ni atteindre la colonne reading au doigt ni ouvrir une carte. | Conserver le défilement par vue. Donner un accès direct aux colonnes et une action explicite sur chaque carte. |
| Note Markdown, éditeur et clavier | Lecture, défilement, passage au source, saisie et effacement ont fonctionné. Le texte d’origine a été vérifié sur disque. | Préserver ce parcours pendant la refonte de la navigation et des erreurs. |
| Navigation assistée | L’outil d’inspection a trouvé les commandes UIKit, mais pas les nœuds ou lignes GPUI dans les écrans parcourus. | Tester la sémantique exposée et faire le parcours avec VoiceOver puis TalkBack. |

Claude a effectué ce tour sur un simulateur distinct après que `DEVICE_IN_USE` a bloqué le pilotage du premier iPhone 18 Pro. Les interactions Android, iPad et du téléphone physique n’ont pas été rejouées dans cet audit.

Sur le Duo simulé, le test de cette revue a ouvert `Overview.base → Wiki` sur l’écran externe puis déplié l’appareil. Le rail est resté à droite et le coffre est apparu à gauche. Les points et la légende étaient visibles dans les deux postures ; aucun nom de note n’était affiché dans le graphe au cadrage initial. Android et iPad restent à vérifier sur appareil dans cette revue.
