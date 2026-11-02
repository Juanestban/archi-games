# ADR 0002: Actualizaciones desde las releases de GitHub

- Estado: aceptado
- Fecha: 2026-11-02

## Contexto

Queremos que la app avise de las versiones nuevas y se actualice sola, como el
launcher de ArtCraft. Pero actualizarse es descargar y ejecutar código, lo que
el ADR 0001 prohíbe en general porque es lo que hace el malware. Este ADR
define la única excepción y sus garantías.

## Decisión

1. **Una sola fuente:** las releases de `github.com/Juanestban/archi-games`,
   por HTTPS (`api.github.com/.../releases/latest` y las descargas de sus
   assets). Ningún otro servidor ni URL configurable.
2. **Nunca en silencio:** la app solo *comprueba* si hay versión nueva (al
   arrancar y cada 6 h, desactivable en Ajustes). Descarga e instala solo
   cuando el jugador pulsa **Actualizar**.
3. **Integridad:** cada release publica `SHA256SUMS.txt`. La descarga se
   verifica contra él antes de ejecutar nada; si no coincide se borra y se
   avisa. Una release sin instalador o sin sumas no se ofrece.
4. **Sin administrador:** la versión instalada se actualiza con el MSI por
   usuario (`msiexec /i ... /passive`); la portable se reemplaza a sí misma
   (renombra el `.exe` en uso, coloca el nuevo y lo arranca).
5. **Las releases las publica CI** (`.github/workflows/release.yml`) a partir
   de un tag `vX.Y.Z` que coincida con la versión de `Cargo.toml`. Nada se
   sube a mano.

## Consecuencias

- Las sumas viven en la misma release que los binarios: protegen de descargas
  corruptas o incompletas, no de alguien que controle la cuenta de GitHub. La
  defensa contra eso es **firmar el ejecutable y el MSI** (fase 2); cuando
  exista, el actualizador también comprobará la firma.
- Sin firma, Windows SmartScreen puede avisar al ejecutar la versión nueva.
- La API de GitHub sin autenticar permite 60 consultas por hora e IP; con una
  cada 6 h no hay riesgo.
