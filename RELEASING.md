# Publicar una versión

Las releases las construye y publica GitHub Actions
(`.github/workflows/release.yml`) cuando subes un tag `vX.Y.Z`. La app instalada
las detecta y ofrece actualizarse ([ADR 0002](docs/adr/0002-actualizaciones-desde-github.md)).

## Pasos

1. Sube la versión en `Cargo.toml` (`[workspace.package] version`), siguiendo
   [SemVer](https://semver.org/lang/es/): parche para arreglos, menor para
   funciones nuevas.
2. Actualiza `Cargo.lock` y comprueba que todo pasa:
   ```
   cargo test --workspace
   cargo clippy --workspace --all-targets
   ```
3. Haz commit de la versión (por ejemplo `chore: versión 0.2.0`) y súbelo.
4. Crea el tag y súbelo:
   ```
   git tag v0.2.0
   git push origin v0.2.0
   ```
5. En la pestaña **Actions** de GitHub, el flujo *Release* comprueba que el tag
   coincide con `Cargo.toml`, ejecuta los tests, empaqueta y publica la
   release con las notas generadas a partir de los commits.

## Qué publica

| Archivo | Para qué |
|---|---|
| `ArchiGames-<versión>-x64.msi` | Instalador por usuario (sin administrador). Es lo que descarga la app instalada al actualizarse. |
| `ArchiGames-<versión>-x64-portable.exe` | Ejecutable suelto, sin instalar. Es lo que descarga la versión portable. |
| `SHA256SUMS.txt` | Sumas SHA-256 que la app verifica antes de instalar. |

**No cambies estos nombres**: el actualizador busca los archivos por ellos.

## Probar el empaquetado en local

```
dotnet tool restore
.\scripts\package.ps1      # -> target\dist
```

## Si algo sale mal

- **El tag no coincide con la versión:** borra el tag (`git tag -d v0.2.0` y
  `git push origin :refs/tags/v0.2.0`), corrige `Cargo.toml` y vuelve a
  crearlo.
- **Una release con fallos:** publica otra versión con el arreglo. Borrar una
  release no "desactualiza" a quien ya la instaló.
