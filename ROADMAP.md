# Roadmap

Diseño completo en [docs/DESIGN.md](docs/DESIGN.md). Marca cada tarea al
terminarla y actualiza **Estado actual**.

## Estado actual

- **Hecho:** fase 1 completa: pasos 1 (`core`), 2 (`storage`), 3 (tracker de Fortnite),
  4 (interfaz), 5 (notificación y sonidos), el rediseño y 6 (bandeja, inicio con Windows,
  instancia única e instalador MSI).
  La cuenta del usuario ya está vinculada y tiene su lectura base.
- **Pendiente del paso 3:** el hito. Tras la próxima partida real del usuario, el
  desbloqueo debe verse en Perfil e Historial.
- **Además:** actualizaciones desde GitHub Releases (ADR 0002), licencia MIT o Apache-2.0 y
  releases automáticas al subir un tag `vX.Y.Z` (ver `RELEASING.md`).
- **Publicado:** [v0.3.0](https://github.com/Juanestban/archi-games/releases/tag/v0.3.0) (MSI, portable y
  `SHA256SUMS.txt`). Comprobado de punta a punta: la v0.2.0 publicada avisa de la 0.3.0 y el
  usuario se actualizó desde la app.
- **En curso:** fase 2. Hecho: lector de repeticiones, paquete "Repeticiones" con 7 logros,
  limpieza de la entrada de inicio con Windows al desinstalar, borrar repeticiones ya leídas
  (preguntando o automático), paquete "career" con 10 logros más de la API,
  Counter-Strike 2 con Game State Integration (la app ya sigue varios juegos) y 16 logros
  más de Fortnite (derribos, distancias, pase de batalla, dispositivos…).
  La API ya funciona con la cuenta real (las lecturas siguen sus partidas) y el lector de
  repeticiones está validado con 3 `.replay` reales (la distancia va en centímetros).
- **Siguiente:** que el usuario active la integración de CS2, reinicie CS2 y juegue una
  partida (debe registrarse al terminar); con eso, publicar la v0.5.0. Después, la tabla de
  armas de Fortnite. Sigue pendiente el hito del paso 3 (un desbloqueo real tras una partida).
- **Última actualización:** 2026-11-03.

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

### Paso 6: integración con el sistema ✅
- [x] Icono en la bandeja (`tray-icon`) con menú Abrir / Consultar ahora / Salir; cerrar la ventana la oculta
- [x] Inicio con Windows vía `HKCU\...\Run`, como interruptor en Ajustes (arranca en la bandeja con `--background`)
- [x] Instancia única (mutex con nombre); una segunda ejecución muestra la ventana de la primera
- [x] Icono propio (dibujado en código) en ventana, bandeja y `.exe`, con nombre y versión en el ejecutable
- [x] Instalador MSI por usuario con WiX 5 (`scripts/package.ps1`), sin permisos de administrador

### Actualizaciones y releases ✅
- [x] Licencia doble MIT o Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`)
- [x] Comprobar la última release de GitHub al arrancar y cada 6 h (desactivable), y desde Ajustes
- [x] Aviso en la app con novedades; descarga verificada con SHA-256 e instalación al pulsar Actualizar
- [x] Versión instalada: MSI por usuario que arranca la nueva versión al terminar; portable: se reemplaza a sí misma
- [x] GitHub Actions: CI (formato, clippy, tests) y release al subir un tag (MSI, portable y `SHA256SUMS.txt`)

## Fase 2

- [x] Leer los `.replay` de Fortnite en solo lectura: estadísticas de fin de partida y puesto
  (`archi_fortnite::replay`), con Fortnite cerrado y solo las posteriores a vincular la cuenta
- [x] Paquete "Repeticiones" (modo `replay`): Demoledor, Ángel de la guarda, Maratonista, Leñador,
  Arquitecto, Ojo de halcón e Intocable (secreto); no cuenta para el platino
- [x] Ajustes muestra cuántas repeticiones se han leído o cómo activarlas
- [x] Preguntar si borrar cada repetición ya leída y opción de borrarlas automáticamente
  (Ajustes > Seguimiento), solo con Fortnite cerrado
- [x] Paquete "career" con 10 logros de estadísticas de la API que el juego base no usaba
- [x] Validar el lector con repeticiones reales del usuario: 3 partidas leídas bien; la
  distancia va en centímetros (antes se adivinaba la unidad)
- [ ] Logros de Fortnite por tipo de arma: los eventos `playerElim` ya se leen; falta saber qué
  número es cada arma
- [x] Varios juegos a la vez: una ranura por juego en el tracker y Ajustes por juego
- [x] ~~CS2 con la API web de Steam~~: descartada, sus contadores están congelados desde CS:GO
  (comprobado con la cuenta real del usuario)
- [x] Counter-Strike 2 con Game State Integration: .cfg en la carpeta de CS2 al activarla,
  servidor en 127.0.0.1:43117 con token, cuenta vinculada sola; 7 logros base con platino
  (incluye As e Imparable) y paquete "arsenal" con 6 logros por arma
- [ ] Probar CS2 de punta a punta con el usuario (activar, reiniciar CS2, jugar una partida)
- [x] Fortnite: derribos y distancia de cada eliminación desde los eventos `playerElim` de las
  repeticiones; nivel del pase de batalla y victorias por dispositivo desde la API
- [x] Condición `reach` (valor actual) para lo que empieza de cero cada temporada
- [x] 16 logros nuevos de Fortnite: 8 en "replays" y 8 en "career"
- [ ] Más juegos (nuevos `GameTracker`)
- [ ] Linux y macOS
- [ ] ~~Firmar el ejecutable y el MSI~~: aplazado por decisión del usuario (2026-11-03). Se publica
  sin firmar, como ArtCraft (comprobado: su v0.41.0 tampoco está firmada); el README explica el
  aviso de SmartScreen. Opciones si se retoma: SignPath Foundation (gratis para código abierto,
  con solicitud) o un certificado OV propio
- [x] Al desinstalar, quitar la entrada de inicio con Windows (solo si es de esa copia)

## Ideas futuras (sin decidir)

- Servidor online: perfiles, amigos, rareza global de logros, anti-trampas
- SDK para que otros desarrolladores integren sus juegos
- Logros de cofres abiertos (10, 50, 100, 250, 500, 1000): no están en los eventos de las
  repeticiones, solo en sus datos de red (replicación de Unreal); leerlos es un proyecto propio
  que se rompería en cada temporada
