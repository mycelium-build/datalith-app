# Coffres locaux et hébergés

La bibliothèque `datalith-vault` s’exécute là où se trouvent les fichiers du coffre. Les documents et les pièces jointes font autorité. Chaque instance peut reconstruire son catalogue et son index de recherche à partir de ces fichiers. [Décision sur le catalogue](../adr/0002-use-a-queryable-catalog-database.md)

| Client | Lecture, sauvegarde et recherche |
|---|---|
| Desktop et mobile | L’application travaille sur le coffre de l’appareil, même hors ligne. Une note enregistrée localement doit pouvoir être rouverte après avoir quitté l’app. |
| Futur client web | Le navigateur affiche les documents et les résultats fournis par le service auto-hébergé. Le service lit, écrit et recherche dans sa copie du coffre. L’accès web nécessite une connexion au service. |

Datalith Sync sera facultatif. Sa première étape sera de répliquer les coffres entre clients desktop et mobiles via le serveur. Une sauvegarde locale réussie et une synchronisation terminée sont deux états distincts. À la reconnexion, si deux appareils ont modifié la même note, les deux modifications doivent être préservées. Le protocole et la résolution des conflits feront l’objet d’une décision ultérieure.

Pour vérifier le refactor actuel, ouvrir un coffre hors ligne sur chaque client natif, modifier une note, relancer l’application et retrouver la modification par la recherche. Une vue partagée doit aussi pouvoir afficher un document ou des résultats fournis par son appelant, de façon à pouvoir les recevoir du serveur dans un futur navigateur. [Découpage en crates](../adr/0004-split-native-applications-into-crates.md)
