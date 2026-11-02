# Roadmap

Diseño completo en [docs/DESIGN.md](docs/DESIGN.md). Marca cada tarea al
terminarla y actualiza **Estado actual**.

## Estado actual

- **Hecho:** pasos 1 (`core`), 2 (`storage`), 3 (tracker de Fortnite), 4 (interfaz) y 5
  (notificación y sonidos).
  La cuenta del usuario ya está vinculada y tiene su lectura base.
- **Pendiente del paso 3:** el hito. Tras la próxima partida real del usuario, el
  desbloqueo debe verse en Perfil e Historial.
- **Siguiente:** paso 6 (bandeja del sistema, inicio con Windows, instancia única, MSI).
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

### Paso 2: `storage` (SQLite) ✅
- [x] Crate `crates/storage` con `rusqlite` (bundled) y migraciones (`PRAGMA user_version`)
- [x] Tablas: lecturas por cuenta, desbloqueos con fecha, ajustes, cuenta vinculada (el progreso se reconstruye, no se guarda)
- [x] Reprocesar el historial cuando se añaden logros nuevos (`rebuild_progress`)
- [x] Registrar cada consulta y sus desbloqueos en una sola transacción (`ingest`)
- [x] XP total y nivel de la cuenta calculados a partir de los desbloqueos (`account_summary`)
- [x] Metalogros de la plataforma, en cascada hasta que no salga ninguno nuevo (`evaluate_platform`)
- [x] Ruta de datos en `%APPDATA%\archi-games\archi-games.db` (crate `directories`)

### Paso 3: tracker de Fortnite
- [x] Trait `GameTracker` y `TrackError` en `core`
- [x] Crate `crates/trackers/fortnite`: cliente de fortnite-api.com (`reqwest` con TLS nativo)
- [x] Mapear las estadísticas de la API a `StatSnapshot` (overall→`all`, solo, duo, trio, squad, ltm)
- [x] Vincular cuenta por nombre de Epic, con aviso si las estadísticas no son públicas
- [x] Detectar el proceso solo por nombre (ToolHelp de Windows, sin abrir ningún proceso)
- [x] Consultas periódicas (`Poller`): 60 s con el juego abierto, cada 30 s durante 5 min al cerrar, 12 min sin el juego
- [x] Cliente simulado (`ReplayTracker`) y tests con lecturas grabadas y una respuesta real de ejemplo
- [x] Consola de desarrollo: `key`, `link`, `status`, `poll`, `run` (sustituida por la interfaz en el paso 4)
- [ ] **Hito:** un logro se desbloquea de verdad tras una partida real

### Paso 4: interfaz (egui) e i18n ✅
- [x] Hilo tokio + canales hacia egui (`worker`: comandos de la UI, eventos con `request_repaint`)
- [x] Pantallas: Perfil, Juegos, Logros, Historial, Ajustes
- [x] Fluent (`fluent-bundle`) para la UI y los logros; idioma del sistema, con el inglés si falta un texto
- [x] Logros secretos y barras de progreso
- [x] Estilo visual "de videojuego" (tema oscuro, acento violeta, insignias por rango)
- [x] Clave de API en el Administrador de credenciales de Windows (`keyring`), migrada desde SQLite
- [x] Test: todo token `ui-…` usado en el código existe en `es` y `en`

### Paso 5: notificación y sonidos ✅
- [x] Viewport egui transparente, sin bordes, siempre encima, que deja pasar los clics
- [x] Animación de entrada, unos 5 s, cola de desbloqueos con el platino al final
- [x] Tarjeta de subida de nivel
- [x] Sonidos por rango y de subida de nivel (`rodio`), sintetizados en código
- [x] Ajustes: volumen y botón "Probar notificación"
- [x] Funciona con la ventana principal minimizada (`RequestPaintWhileHidden`)

### Rediseño (entre los pasos 5 y 6) ✅
- [x] Estilo de ArtCraft: superficies casi negras, filos finos, Archivo/Inter/Geist Mono, esquinas de 3 px
- [x] Ventana sin bordes con barra de título propia, botones de ventana y redimensionado por los bordes
- [x] Barra lateral con iconos vectoriales, indicador animado y tarjeta de cuenta con XP
- [x] Cabecera con migas de pan e indicador de estado del seguimiento
- [x] Animaciones: transición entre pantallas, barras y anillos que se llenan, brillos al pasar el ratón
- [x] Notificación rediseñada: franja del rango, destello que la cruza y línea de tiempo restante
- [x] Sonidos nuevos: campanas cristalinas en estéreo con reverberación, una variación por rango

### Paso 6: integración con el sistema
- [ ] Icono en la bandeja (`tray-icon`); cerrar la ventana la oculta
- [ ] Inicio con Windows vía `HKCU\...\Run`
- [ ] Instancia única
- [ ] Instalador MSI (`cargo-wix`)

## Fase 2

- [ ] Analizar los `.replay` de Fortnite en solo lectura, para logros por partida más finos
- [ ] Logros de cofres abiertos (10, 50, 100, 250, 500, 1000) en un `pack` aparte, **solo si** las
  repeticiones permiten atribuir cada cofre al jugador (posición del jugador cuando el cofre pasa a
  abierto). Ninguna API da los cofres; ver DESIGN, "Pendiente de investigar"
- [ ] Más juegos (nuevos `GameTracker`)
- [ ] Linux y macOS
- [ ] Firmar el ejecutable

## Ideas futuras (sin decidir)

- Servidor online: perfiles, amigos, rareza global de logros, anti-trampas
- SDK para que otros desarrolladores integren sus juegos
