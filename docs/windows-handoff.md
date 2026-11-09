# Passation : distribution Windows / WinGet

État au 9 novembre 2026 : [PR brouillon #67](https://github.com/mycelium-build/datalith-app/pull/67),
branche `feat/winget-distribution`. Aucun test Windows exécuté lors de la préparation
sur Linux. Aucun manifest soumis à Microsoft. Les détails et sources officielles
sont dans [windows-distribution.md](windows-distribution.md).

## Objectif et contraintes

Valider les installateurs NSIS upstream existants avant de préparer WinGet.
Conserver les versions upstream, tags et assets publiés. Aucun rebuild dans le
futur dépôt distribution. Corrections packaging dans une PR app, sans fusion ;
utiliser `fix(windows): …` si un défaut est démontré (patch 1.1.2 envisagé).
Ne pas modifier les protections. Pas de soumission Microsoft sans revue utilisateur.

Signature : **gratuite uniquement**. Examiner SignPath Foundation et ses conditions
avant toute proposition d'inscription. Aucun achat, compte ou certificat à créer.
Le but est de réduire les alertes SmartScreen ; ni Authenticode ni WinGet ne
promettent leur disparition. Ne pas confondre signature updater `.sig`, signature
Authenticode, réputation SmartScreen et détection antivirus.

## Préparer l'environnement

Utiliser une VM Windows jetable avec snapshot, un utilisateur de test sans données
Datalith et PowerShell 7 natif. Tester x64 et ARM64 sur leurs machines respectives.
Installer Git, GitHub CLI et WinGet ; consigner version/build Windows, architecture,
`$PSVersionTable`, `winget --info`, état SmartScreen/Smart App Control et politiques
entreprise éventuelles. Garder les protections actives pour mesurer SmartScreen.

```powershell
git clone https://github.com/mycelium-build/datalith-app.git
Set-Location datalith-app
git switch --track origin/feat/winget-distribution
git rev-parse HEAD
```

Ne jamais lancer le script de validation sur une installation personnelle : il
installe puis désinstalle Datalith. Il refuse une installation existante.

## Vérifier les assets publiés

Exemple pour la baseline stable 1.1.1, à adapter explicitement pour une future release :

```powershell
$tag = 'v1.1.1'
$version = $tag.Substring(1)
$arch = 'x64' # arm64 sur Windows ARM64
$name = "datalith_${version}_${arch}-setup.exe"
$release = gh api "repos/mycelium-build/datalith-app/releases/tags/$tag" | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $release.draft -or $release.prerelease) { throw 'Release invalide' }
$asset = @($release.assets | Where-Object name -CEQ $name)
if ($asset.Count -ne 1) { throw 'Asset absent ou ambigu' }
gh release download $tag --repo mycelium-build/datalith-app --pattern $name
if ($LASTEXITCODE -ne 0) { throw 'Téléchargement échoué' }
$hash = (Get-FileHash $name -Algorithm SHA256).Hash.ToLowerInvariant()
if ("sha256:$hash" -cne $asset[0].digest) { throw 'SHA-256 incorrect' }
Get-AuthenticodeSignature $name | Format-List Status, StatusMessage, SignerCertificate, TimeStamperCertificate
./scripts/windows/test-installer.ps1 -Installer $name -Version $version -Architecture $arch
```

Le script vérifie `/S`, réinstallation de la même version, registre HKCU,
architecture PE, identité/version du binaire et désinstallation `/S`.
Cela ne valide ni une mise à jour entre versions ni SmartScreen.
Le workflow manuel `windows-installer.yml` réalise les mêmes vérifications sur
runners x64/ARM64. Un nouveau workflow peut devoir exister sur la branche par
défaut avant d'être déclenchable : ne pas fusionner la PR pour contourner cela ;
les commandes locales suffisent pour la passation.

## Compléter les vérifications manuelles

1. Sur snapshot propre, installer en interface graphique puis avec `/P` : aucune
   interaction pour `/P`, aucun lancement applicatif imprévu en mode silencieux.
   Vérifier démarrage, raccourcis, licences embarquées et désinstallation Windows.
2. Relever dans la vue registre 64 bits
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Datalith` :
   DisplayName, Publisher, DisplayVersion, InstallLocation et UninstallString.
   Attendus : Datalith, Mycelium Build, version sans `v`, installation utilisateur
   sous `%LOCALAPPDATA%\Datalith`. Confirmer l'absence d'installation machine.
3. Installer une ancienne stable publiée, créer des données de test identifiables,
   fermer l'app puis installer la candidate. Vérifier version, conservation des
   données et absence de doublon dans Applications installées. Refaire app ouverte
   en enregistrant les données auparavant : relever fermeture/échec/dialogue.
4. Vérifier que désinstaller ne supprime pas les données utilisateur ; réinstaller
   et confirmer leur récupération. Tester aussi le canal Preview en coexistence
   si les correctifs touchent la configuration commune.
5. Une fois les manifests préparés dans le dépôt distribution, utiliser
   `winget validate --manifest <dossier-version>` puis, dans la VM uniquement,
   activer `winget settings --enable LocalManifestFiles` depuis une console admin.
   Avec le compte de test, exécuter `winget install --manifest <dossier-version> --silent`,
   vérifier `winget list --id MyceliumBuild.Datalith --exact`, installer la version
   précédente et tester `winget upgrade --manifest <dossier-version> --silent`, puis
   `winget uninstall --id MyceliumBuild.Datalith --exact --silent`.
   Conserver sorties et logs ; adapter l'ID si la revue des manifests le change.

## Mesurer SmartScreen séparément

Depuis un snapshot Windows propre connecté à Internet, télécharger l'EXE officiel
avec Edge, conserver le fichier tel quel et lancer depuis l'Explorateur. Un download
par CLI ou un runner CI ne reproduit pas nécessairement le parcours utilisateur.
Consigner URL, heure, SHA-256, version Edge, état des protections, résultat de
`Get-AuthenticodeSignature` et présence de la provenance Internet :

```powershell
Get-Content -LiteralPath "$HOME\Downloads\datalith_1.1.1_x64-setup.exe" -Stream Zone.Identifier
```

Ne pas utiliser `Unblock-File`, supprimer Zone.Identifier ou désactiver SmartScreen
pour déclarer le test réussi. Capturer le texte exact et une capture d'écran de
l'avertissement : téléchargement Edge, « application non reconnue » SmartScreen,
Smart App Control ou détection Defender avec nom de menace sont des cas distincts.
L'absence de flux Zone.Identifier doit être signalée dans le résultat.
Une alerte de réputation n'établit pas qu'un malware a été détecté.

Après une éventuelle signature gratuite approuvée, répéter sur une nouvelle release
et un snapshot propre. Vérifier la signature de l'installateur, de l'exécutable
installé et de l'uninstaller. Ne jamais re-signer les assets déjà publiés.

## Retour attendu et suite

Rapporter pour chaque architecture : commit testé, Windows/WinGet, tag, URL/hash,
signature, codes de sortie, métadonnées registre, résultats install/réinstall/
upgrade/uninstall, conservation des données et captures SmartScreen. Marquer
explicitement chaque cas non testé. Joindre les logs sans secrets ni données privées.

Le dépôt `mycelium-build/windows-packages`, le fork éventuel `winget-pkgs`, les
manifests et l'automatisation multi-app restent à implémenter. Les updates doivent
ouvrir des PR internes après release stable, avec jeton App limité au dépôt et
identité `datalith-bot[bot]` (ID 311197260). Les secrets existants ne sont pas
implicitement disponibles dans un nouveau dépôt. Lier chaque PR via T3 après
création/push ; aucune soumission Microsoft, fusion ou changement de protection.
