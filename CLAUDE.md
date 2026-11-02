# Archi Games

Plataforma de escritorio de logros para Fortnite (y más juegos después), en
Rust nativo con egui. El usuario habla español: responder en español; el
código y los identificadores van en inglés.

## Retomar sesión

1. Leer `ROADMAP.md` → "Estado actual" dice qué está hecho y qué sigue.
2. Las decisiones de diseño están en `docs/DESIGN.md`. **No volver a preguntar
   lo ya decidido**; si algo debe cambiar, proponerlo y actualizar el documento.
3. Para entender el código, consultar primero el grafo de graphify antes de
   leer archivos sueltos (ver abajo).

## graphify

- El grafo vive en `graphify-out/` (`graph.json`, `GRAPH_REPORT.md`,
  `graph.html`). No está en git.
- Para preguntas sobre el código: `graphify query "<pregunta>"`,
  `graphify explain "<nodo>"` o `graphify path "A" "B"`. Leer archivos solo
  para lo que el grafo no responda.
- **Código** (`.rs`, `Cargo.toml`): un hook `Stop` en
  `.claude/settings.local.json` ejecuta `graphify update` al final de cada
  turno. No hace falta hacer nada.
- **Documentos** (`.md`): el hook no los cubre porque requieren extracción
  semántica. Después de cambiar cualquier `.md`, ejecutar `/graphify . --update`
  antes de terminar la tarea.
- `achievements.toml` y `.ftl` no los indexa graphify.

## Reglas

- [ADR 0001](docs/adr/0001-sin-interaccion-con-procesos-de-juegos.md) es
  obligatorio: nunca tocar los procesos de los juegos.
- `core` no sabe de ningún juego concreto.
- Todo texto visible es un token Fluent con traducción en `es` y `en`.
- Antes de dar algo por hecho: `cargo test --workspace` y
  `cargo clippy --workspace --all-targets` sin avisos.
- Al terminar una tarea: marcarla en `ROADMAP.md`, actualizar "Estado actual"
  y actualizar el grafo de graphify.
