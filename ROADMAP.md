# Roadmap

Diseño completo en [docs/DESIGN.md](docs/DESIGN.md). Marca cada tarea al
terminarla y actualiza **Estado actual**.

## Estado actual

- **Hecho:** paso 1 (`core`).
- **Siguiente:** paso 2 (`storage`).
- **Última actualización:** 2026-11-02.

## Fase 1: versión inicial con Fortnite

### Paso 1: workspace y `core` ✅
- [x] Workspace de crates (`crates/core`, `crates/app`)
- [x] `StatSnapshot`: estadísticas genéricas por modo, con diferencia entre lecturas
- [x] Definiciones TOML con validación (IDs, platino, tokens Fluent, comparadores)
- [x] Condiciones `total`, `match` y `streak`
- [x] Platino automático: el último en notificarse, sin contar los paquetes
- [x] XP por rango y curva de niveles configurables (`Progression`)
- [x] Logros de ejemplo de Fortnite y de la plataforma en es/en, con test de tokens
- [x] Tests de escenarios con secuencias de lecturas simuladas
- [x] ADR 0001 (ciudadano limpio)

### Paso 2: `storage` (SQLite)
- [ ] Crate `crates/storage` con `rusqlite` (bundled) y migraciones
- [ ] Tablas: lecturas (snapshots), progreso por juego, desbloqueos con fecha, ajustes, cuenta vinculada
- [ ] Reprocesar el historial cuando se añaden logros nuevos
- [ ] XP total y nivel de la cuenta calculados a partir de los desbloqueos
- [ ] Estadísticas de los metalogros (platinums, level, unlocked)
- [ ] Ruta de datos en `%APPDATA%\archi-games` (crate `directories`)

### Paso 3: tracker de Fortnite
- [ ] Trait `GameTracker` en `core`
- [ ] Crate `crates/trackers/fortnite`: cliente de fortnite-api.com (`reqwest`)
- [ ] Mapear las estadísticas de la API a `StatSnapshot` (overall→`all`, solo, duo, trio, squad, ltm)
- [ ] Vincular cuenta por nombre de Epic, con aviso si las estadísticas no son públicas
- [ ] Detectar el proceso solo por nombre (`sysinfo` refrescando únicamente nombres)
- [ ] Consultas periódicas: 60 s con el juego abierto, ráfaga de unos 5 min al cerrar, 10–15 min sin el juego
- [ ] Cliente simulado y tests con lecturas grabadas
- [ ] **Hito:** un logro se desbloquea de verdad, comprobado en consola

### Paso 4: interfaz (egui) e i18n
- [ ] Hilo tokio + canales hacia egui
- [ ] Pantallas: Perfil, Juegos, Logros, Historial, Ajustes
- [ ] Fluent (`fluent-bundle`) para la UI y los logros; idioma del sistema, con el inglés si falta un texto
- [ ] Logros secretos y barras de progreso
- [ ] Estilo visual "de videojuego"

### Paso 5: notificación y sonidos
- [ ] Viewport egui transparente, sin bordes, siempre encima, que deja pasar los clics
- [ ] Animación de entrada, unos 5 s, cola de desbloqueos con el platino al final
- [ ] Tarjeta de subida de nivel
- [ ] Sonidos por rango y de subida de nivel (`rodio`)

### Paso 6: integración con el sistema
- [ ] Icono en la bandeja (`tray-icon`); cerrar la ventana la oculta
- [ ] Inicio con Windows vía `HKCU\...\Run`
- [ ] Instancia única
- [ ] Instalador MSI (`cargo-wix`)

## Fase 2

- [ ] Analizar los `.replay` de Fortnite en solo lectura, para logros por partida más finos
- [ ] Más juegos (nuevos `GameTracker`)
- [ ] Linux y macOS
- [ ] Firmar el ejecutable

## Ideas futuras (sin decidir)

- Servidor online: perfiles, amigos, rareza global de logros, anti-trampas
- SDK para que otros desarrolladores integren sus juegos
