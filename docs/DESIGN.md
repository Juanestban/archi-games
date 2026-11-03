# Diseño de Archi Games

Decisiones acordadas en la sesión de diseño (grill-me) del 2026-11-02. Cada
sección dice **qué** se decidió y **por qué**. Si algo cambia, se actualiza
aquí y, si es una decisión de arquitectura, se escribe un ADR en `docs/adr/`.

## 1. Producto

- **Qué es:** una plataforma de escritorio de logros estilo PlayStation para
  juegos-servicio que no tienen sistema de logros. Primer juego: **Fortnite**.
- **Para quién:** jugadores completistas. Nace como proyecto propio
  (aprendizaje y portafolio) con la intención de darle a Fortnite objetivos
  que completar.
- **Logros "independientes":** son logros **distintos** a los que ofrece el
  juego, creados por la plataforma. No se importan logros existentes.
- **Quién crea los logros:** solo el autor, en archivos TOML. No hay editor ni
  logros de la comunidad.
- **Sin desafíos temporales:** nada diario, semanal ni por temporada. Solo
  logros permanentes.
- **Metalogros de la plataforma:** "primer platino", "nivel 10", "100 logros".
  Dan XP como cualquier logro.

## 2. Datos y detección

- **Fuente:** API de terceros **fortnite-api.com** (Epic no tiene API pública
  oficial de estadísticas). Cada usuario pone su propia clave de API en los
  ajustes.
- **Vinculación:** el usuario escribe su nombre de Epic y la app resuelve su
  ID de cuenta. Si sus estadísticas no son públicas en Fortnite, la app avisa.
- **"Tiempo real" = al terminar cada partida.** La API solo se actualiza al
  acabar la partida. Desbloquear algo durante la partida exigiría tocar el
  proceso del juego, y eso está prohibido (ver §7).
- **Cuentan todas las partidas de la cuenta**, también las de consola: las
  estadísticas son de la cuenta de Epic, no del PC.
- **Frecuencia de consulta:**
  - Fortnite abierto en PC (`FortniteClient-Win64-Shipping.exe`): cada 60 s.
  - Al cerrar el juego: una ráfaga de consultas durante unos 5 min, por si la
    API tarda en actualizarse.
  - Sin el juego abierto: cada 10–15 min, para cubrir las partidas en consola.
  - Respetar siempre los límites de la API.
- **Detalle por partida:** en la fase 1 solo hay totales de la API; lo que
  pasó en una partida se deduce comparando dos lecturas. En la fase 2 se
  analizarán los `.replay` locales para logros más finos (arma, pico,
  colocación…).
- **El progreso empieza en la instalación:** la primera lectura es la base y
  el historial anterior no cuenta. Los metalogros empiezan desde cero.
- **Todas las lecturas se guardan.** Un logro nuevo se evalúa reprocesando el
  historial desde la instalación.

### Persistencia (decidido al implementar el paso 2)

- **Las lecturas se guardan por cuenta.** Si vinculas otra cuenta de Epic,
  empieza desde su propia base y no se mezcla con la anterior. Los logros ya
  desbloqueados se conservan.
- **Lecturas idénticas no se guardan.** Una consulta sin cambios no altera el
  motor, así que omitirla reduce el historial sin cambiar el resultado al
  reprocesar.
- **El progreso no se guarda: se reconstruye** reprocesando las lecturas al
  arrancar. Los desbloqueos que el historial gana por un logro nuevo se fechan
  con la lectura que los ganó.
- **Un desbloqueo nunca se revoca**, aunque luego endurezcas su condición o lo
  vuelvas a evaluar. Si borras un logro del TOML, deja de contar para la XP.
- **Metalogros:** la estadística `unlocked` cuenta solo los logros de juegos,
  no los de la plataforma, para que no se alimenten a sí mismos. `level` sí
  incluye la XP de la plataforma, por eso se evalúan en cascada.

### Tracker de Fortnite (decidido al implementar el paso 3)

- **Endpoint:** `GET https://fortnite-api.com/v2/stats/br/v2?name=<nombre>` para
  vincular y `.../v2/stats/br/v2/<accountId>` para cada consulta, con la clave
  en la cabecera `Authorization`. Se usan las estadísticas `all` (todos los
  dispositivos, consola incluida), de por vida.
- **Estadísticas:** solo contadores (`score`, `wins`, `top3`…`top25`, `kills`,
  `deaths`, `matches`, `minutes_played`, `players_outlived`); los cocientes
  (`kd`, `winRate`…) no. Cada modo trae distintos `top*`: solo `top10`/`top25`,
  dúo `top5`/`top12`, escuadrón `top3`/`top6`; `overall` los trae todos.
- **La API nunca rellena `trio`**: los logros de tríos no se pueden conseguir
  hasta que cambie.
- **Una respuesta incompleta es un error, no una lectura.** Si faltan
  `overall` o `matches`, no se guarda nada: una lectura vacía se leería como
  una bajada y luego un salto, y regalaría logros.
- **Errores:** 403 con "not public" → estadísticas privadas (se explica cómo
  activarlas: *Mostrar en la clasificación de carrera*); 401/403 → clave
  inválida; 404 → cuenta inexistente o sin partidas; 429 → esperar
  `Retry-After` o 5 min. Un fallo de red se reintenta en la siguiente consulta.
- **Detección del proceso:** con la API ToolHelp de Windows
  (`CreateToolhelp32Snapshot`), que lista nombres sin abrir ningún proceso.
  Sustituye a `sysinfo`, que no garantiza no abrir handles (ADR 0001). Se
  comprueba cada 5 s; solo se busca `FortniteClient-Win64-Shipping.exe`.
- **Frecuencia concreta:** 60 s con el juego abierto; al cerrarlo, una
  consulta inmediata y luego cada 30 s durante 5 min; sin el juego, cada 12 min.
- **TLS nativo** (schannel en Windows): sin OpenSSL ni compilar criptografía
  en C.
- **La clave de API** se guarda en el Administrador de credenciales de Windows
  (`keyring`), nunca en SQLite. Si una versión anterior la dejó en `settings`,
  se mueve al arrancar.
- **La consola de desarrollo** del paso 3 se eliminó en el paso 4: todo se hace
  desde la interfaz.

### Interfaz (decidido al implementar el paso 4)

- **eframe 0.37** con su renderizador por defecto (wgpu). Las compilaciones de
  release no abren consola (`windows_subsystem = "windows"`).
- **Dos conexiones a SQLite:** la UI solo lee (y guarda el idioma); el hilo del
  tracker escribe. WAL permite leer mientras se escribe. La UI abre (y migra)
  la base antes de arrancar el hilo.
- **El hilo del tracker** recibe `Command`s (guardar clave, vincular, consultar
  ahora) y envía `Event`s (estado, progreso, desbloqueos, resultado de la
  vinculación). Los errores llegan como `Problem`, que la UI traduce.
- **Textos de la interfaz** en `crates/app/locales/{es,en}.ftl` con prefijo
  `ui-`; los de cada juego siguen en `games/<id>/locales`. Todo va a un único
  bundle Fluent por idioma.
- **Fechas relativas** ("hace 5 min") hasta una semana; después, la fecha en
  UTC (sin dependencias de zonas horarias).
- **Progreso de los metalogros:** se calcula a partir del resumen de la cuenta
  (`AccountSummary::platform_stats`), igual que al evaluarlos.
- **Porcentaje completado** de un juego = logros desbloqueados / total, con el
  platino incluido (sin ponderar por rango).
- **Inicio con Windows** se añadirá en Ajustes con el paso 6.

### Notificación y sonidos (decidido al implementar el paso 5)

- **Ventana de notificación:** viewport inmediato de egui de 400×96 a 24 px del
  borde derecho y 72 px del inferior (deja libre la barra de tareas) del monitor
  principal. Sin bordes, transparente, siempre encima, deja pasar los clics, no
  aparece en la barra de tareas y no roba el foco. Solo existe mientras hay
  tarjetas en cola.
- **Tarjetas:** entran deslizándose (0,35 s), duran 5 s y se desvanecen
  (0,4 s). Orden: desbloqueos, platinos al final y la subida de nivel la última.
  El platino lleva un borde que late.
- **Con la ventana principal minimizada u oculta** las notificaciones siguen
  saliendo: mientras hay cola, `logic` pide `RequestPaintWhileHidden`.
- **Sonidos sintetizados** en código (tonos de campana con envolvente), sin
  archivos de audio ni licencias: más notas cuanto más alto el rango; el platino
  acaba en un acorde. Suenan al aparecer su tarjeta. Sin dispositivo de audio,
  se omiten.
- **Volumen** 0–100 (por defecto 70) en Ajustes, con un botón "Probar
  notificación" que muestra una tarjeta de oro, una de platino y una de nivel.
- **Desbloqueos retroactivos** (logros nuevos que el historial ya cumplía)
  también se notifican, al arrancar.

### Rediseño visual y sonoro (entre los pasos 5 y 6)

- **Base: el sistema de ArtCraft** (`storytold/artcraft`, MIT/Apache-2.0; ver
  `NOTICE.md`): superficies casi negras (`#121316`, cromo `#0a0a0c`), filos
  blancos al 15 %, esquinas de 3 px solo en lo pulsable, tarjetas cuadradas,
  azul `#2d81ff` para lo principal. Encima, los colores de los rangos.
- **Tipografías** (OFL, incluidas en `crates/app/assets/fonts`): Archivo ancha
  para títulos y números, Inter para el texto y Geist Mono en mayúsculas
  espaciadas para etiquetas, botones y cifras.
- **Ventana sin bordes:** barra de título propia (arrastrar, doble clic para
  maximizar, botones de Windows) y redimensionado desde los bordes.
- **Estructura:** barra lateral pegada al borde (Biblioteca arriba; Ajustes y la
  tarjeta de cuenta abajo), cabecera con migas de pan y un indicador de estado
  ("Siguiendo", "En partida", "Consultando…", "Revisa Ajustes").
- **Animaciones:** la pantalla entra con un fundido y un leve ascenso (0,28 s);
  barras y anillos se llenan al entrar (0,8 s); el indicador de la barra
  lateral se desliza; las filas y los botones se iluminan al pasar el ratón.
- **Iconos** dibujados con vectores (estilo Lucide), sin imágenes.
- **Notificación:** tarjeta cuadrada con franja del color del rango, trofeo,
  etiqueta mono, nombre en Archivo, una banda de luz que la cruza (en el
  platino, una y otra vez) y una línea que marca el tiempo restante.
- **Sonidos:** campanas de vidrio con parciales inarmónicos, un soplo de aire
  previo, estéreo con ligera desafinación y una reverberación Schroeder.
  Melodías originales (no se copia ningún sonido de consola): bronce, una nota;
  plata, una quinta ascendente; oro, un arpegio; platino, un florecimiento
  lento con cascada y acorde final de unos 4 s; subida de nivel, una escala
  rápida. Se renderizan una vez en segundo plano y se mezclan sin bloquear la
  UI.

### Integración con Windows (decidido al implementar el paso 6)

- **Bandeja:** `tray-icon`, creada en el hilo principal. Clic izquierdo abre la
  ventana; el menú tiene Abrir, Consultar ahora y Salir (traducidos, se
  actualizan al cambiar de idioma). Cerrar la ventana (botón o Alt+F4) la
  oculta; solo "Salir" termina la app. Sin bandeja, cerrar sale.
- **Inicio con Windows: opcional, apagado por defecto.** Cambia lo decidido en
  §5: una app que se añade sola a `Run` sin preguntar es lo que hace el malware
  para persistir, así que lo decide el usuario con un interruptor en Ajustes.
  La entrada guarda la ruta del `.exe` con `--background`, que arranca con la
  ventana oculta (se oculta en el primer frame porque eframe la muestra tras
  él).
- **Instancia única:** mutex con nombre `Local\ArchiGames.Instance`. Una
  segunda ejecución activa el evento `Local\ArchiGames.Show` y sale; la
  primera muestra su ventana.
- **Icono:** un trofeo blanco sobre el azul de la marca, dibujado en código
  (`icon.rs`). `build.rs` lo convierte en `.ico` y lo incrusta con nombre y
  versión (`winresource`); sin el compilador de recursos del SDK solo avisa.
- **Instalador:** WiX 5 como herramienta local de .NET (`dotnet-tools.json`;
  la v6 exige aceptar una EULA). MSI por usuario en
  `%LOCALAPPDATA%\Programs\Archi Games` con acceso directo en el menú Inicio;
  sin administrador. `cargo-wix` no sirve: solo soporta WiX 3.
- **Desinstalar:** el MSI ejecuta la copia instalada con `--uninstall` antes de
  borrar archivos, solo en una desinstalación real (no al actualizar), y esta
  borra la entrada de `Run` si arranca esa misma copia (la de una portable se
  respeta). Si falla, la desinstalación sigue. Los datos (SQLite y la clave en
  el Administrador de credenciales) se conservan. Firmar el ejecutable sigue en
  la fase 2.

### Actualizaciones y releases

- **Como ArtCraft:** la app consulta la última release de GitHub, avisa con una
  franja sobre el contenido (novedades y botón Actualizar) y se actualiza sola.
  Reglas de seguridad en el [ADR 0002](adr/0002-actualizaciones-desde-github.md).
- **Cuándo comprueba:** 15 s después de arrancar y cada 6 h, si está activado
  en Ajustes > Acerca de (por defecto sí); también con "Buscar
  actualizaciones". Las comprobaciones automáticas fallan en silencio.
- **Qué descarga:** si el `.exe` está en `%LOCALAPPDATA%\Programs\Archi Games`
  (instalado con el MSI), el MSI nuevo, que se instala sin administrador y
  arranca la versión nueva (`LAUNCHAPP=1`); si no, el `.exe` portable, que
  reemplaza al actual. La versión nueva arranca con `--after-update` y espera
  a que la anterior cierre antes de tomar la instancia única.
- **Versiones:** `vX.Y.Z` estrictas; las pre-releases no se ofrecen.
- **Releases:** las publica GitHub Actions al subir un tag que coincida con
  `Cargo.toml` (ver `RELEASING.md`). Nombres fijos de los archivos, porque el
  actualizador los busca por ellos.
- **Licencia:** MIT o Apache-2.0, a elección de quien use el código, igual que
  ArtCraft.

## 3. Modelo de logros

- **Rangos y XP** (puntos de trofeo de PlayStation): bronce 15, plata 30,
  oro 90, platino 300.
- **Niveles:** pasar del nivel *n* al *n+1* cuesta `100 · n^1.5` XP. Se empieza
  en el nivel 1 y no hay máximo. Los valores son configurables sin recompilar.
- **Platino:** se desbloquea solo al completar todos los logros del **juego
  base**. Los logros nuevos van en **paquetes** (`pack`), que no afectan al
  platino. Un platino no se pierde nunca.
- **Logros secretos** (`hidden`): no se ve qué hay que hacer hasta
  desbloquearlos.
- **Barras de progreso** en los logros de tipo `total` (p. ej. 37/100). **No**
  hay notificaciones de progreso parcial.
- **Tipos de condición:**
  - `total`: acumulado desde la instalación, `stat >= target`.
  - `match`: en una sola partida. **Solo** se evalúa si entre dos lecturas hubo
    exactamente 1 partida en ese modo; nunca se da un logro dudoso.
  - `streak`: N partidas seguidas cumpliendo `all`. Una derrota o una lectura
    con varias partidas reinicia la racha.
- **Modos:** `all` (total), `solo`, `duo`, `trio`, `squad`, `ltm`.
- **Traducciones:** títulos y descripciones son **tokens Fluent** traducidos en
  `locales/{es,en}.ftl`. El idioma se detecta del sistema y se puede cambiar
  en Ajustes; si falta un texto, se usa el inglés. Un test comprueba que no
  falte ningún token.

## 4. Arquitectura

- **Rust nativo, sin Tauri.** Interfaz con **egui/eframe**.
- **Un solo programa** en la bandeja del sistema, organizado como workspace:

  | Crate | Responsabilidad |
  |---|---|
  | `core` | Definiciones, evaluación, XP y niveles. No sabe nada de ningún juego. |
  | `storage` | SQLite (`rusqlite`): lecturas, progreso, desbloqueos con fecha, ajustes. |
  | `trackers/fortnite` | Implementa el trait `GameTracker`: detectar el proceso, obtener y normalizar las estadísticas. |
  | `ui` / `app` | egui, notificaciones, bandeja, sonidos. |

- **Hilos:** egui en el hilo principal; un runtime **tokio** en otro hilo para
  el tracker (`reqwest`). Se comunican por canales, y cada desbloqueo despierta
  a la UI con `ctx.request_repaint()`.
- **Nuevos juegos:** cada uno implementa `GameTracker`. El motor solo ve
  `StatSnapshot`s genéricos.
- **Datos:** definiciones en `games/<id>/achievements.toml`, estado en SQLite
  dentro de `%APPDATA%\archi-games`.
- **Un solo perfil** por instalación. Sin protección anti-trampas en local; si
  algún día hay servidor, la validación irá allí.
- **Plataformas:** Windows primero; Linux y Mac en el futuro (por eso egui y
  nada exclusivo de Windows en `core`).

## 5. Interfaz

- **Pantallas:**
  - **Perfil:** nivel, barra de XP, número de logros por rango.
  - **Juegos:** porcentaje completado y platino de cada juego.
  - **Logros:** lista del juego con filtros (desbloqueados, pendientes, rango).
  - **Historial:** desbloqueos recientes.
  - **Ajustes:** nombre de Epic, clave de API, idioma, volumen, inicio con
    Windows.
- **Comportamiento:** puede iniciarse con Windows (opcional, ver paso 6), en la bandeja. Cerrar
  la ventana solo la oculta. Instancia única.

## 6. Notificaciones y sonido

- **Ventana egui propia:** sin bordes, transparente, siempre encima, en la
  esquina inferior derecha, y deja pasar los clics al juego. Funciona en modo
  ventana y en pantalla completa sin bordes, pero no en pantalla completa
  exclusiva.
- **Contenido:** entra deslizándose y dura unos 5 s. Muestra el icono del
  rango, el título traducido y la XP ganada. Si subes de nivel, aparece una
  segunda tarjeta.
- **Cola:** si llegan varios desbloqueos a la vez, se muestran uno tras otro.
  El platino va siempre el último y con efecto especial.
- **Sonidos** (`rodio`): uno por rango y otro al subir de nivel.

## 7. Reglas de ciudadano limpio

Obligatorias y detalladas en
[ADR 0001](adr/0001-sin-interaccion-con-procesos-de-juegos.md). En resumen:
nunca abrir handles al juego, leer su memoria, inyectar código ni usar hooks;
detectar el proceso solo por nombre; sin privilegios de administrador; inicio
con Windows solo vía `HKCU\...\Run`; sin empaquetadores ni ofuscación;
instalador MSI y, a medio plazo, ejecutable firmado.

## 8. Pruebas

- Un **cliente de API simulado** reproduce secuencias de lecturas grabadas
  (p. ej. "victoria con 12 eliminaciones").
- Con él se prueban `total`, `match`, `streak`, el platino y la XP sin
  depender de Fortnite.

### Repeticiones (fase 2, decidido con el usuario)

- **Fuente:** los `.replay` que Fortnite guarda en PC en
  `%LOCALAPPDATA%\FortniteGame\Saved\Demos` si el jugador activa *Ajustes >
  Juego > Grabar repeticiones*. Las partidas en consola no generan repeticiones,
  así que estos logros son solo de PC.
- **Qué se lee:** solo la cabecera y los eventos `AthenaMatchStats`
  (precisión, asistencias, eliminaciones, daño con armas y otro daño,
  reanimaciones, daño recibido, daño a estructuras, materiales recogidos y
  usados, distancia) y `AthenaMatchTeamStats` (puesto). Los datos de red se
  saltan sin interpretarlos. Los eventos van cifrados con AES-256-ECB con una
  clave que está en el propio archivo. Formato según FortniteReplayDecompressor
  (MIT).
- **Cuándo:** con Fortnite cerrado, cada 2 min y unos 35 s después de cerrarlo;
  solo archivos sin modificar en 30 s, en modo solo lectura (ADR 0001). Solo
  cuentan las repeticiones posteriores a vincular la cuenta. Cada archivo
  (nombre y tamaño) se lee una vez; uno ilegible o sin estadísticas se marca
  como leído sin sumar nada.
- **En el motor:** cada repetición suma al modo `replay` de las lecturas de
  Fortnite (`matches`, `wins` si quedó 1.º, `eliminations`, `assists`, `damage`,
  `other_damage`, `damage_taken`, `structure_damage`, `revives`,
  `materials_gathered`, `materials_used`, `distance_m`, `accuracy` en % entero).
  Los totales se suman también a cada lectura de la API, para que el modo no
  "baje" entre consultas. Así `total`, `match` y `streak` funcionan sin cambios.
  El archivo y su lectura se guardan en la misma transacción (tabla `replays`).
- **Distancia:** el formato no documenta su unidad. Si una partida supera
  50 000 se toma como centímetros (nadie recorre 50 km en una partida); se
  confirmará con repeticiones reales.
- **Cuenta:** las repeticiones de este PC cuentan para la cuenta vinculada; si
  otra persona juega en el mismo PC con otra cuenta, sus partidas también
  sumarían.
- **Paquete `replays`:** no cuenta para el platino, para que quien juega en
  consola pueda completar el juego base.
- **Fuera por ahora:** cofres (solo en los datos de red, ver Ideas futuras) y
  logros por tipo de arma (los números de arma de `playerElim` cambian entre
  temporadas; se harán con repeticiones reales).
- **Borrar repeticiones ya leídas** (pedido por el usuario, para ahorrar disco):
  tras leerlas, un aviso arriba de la ventana ofrece *Borrar*, *Conservar* o
  *Borrar siempre*. Ajustes > Seguimiento tiene el interruptor "Borrar las
  repeticiones después de analizarlas" (`replays.auto_delete`, apagado por
  defecto). Solo se ofrecen o borran las que sumaron estadísticas (las
  ilegibles se conservan para una versión futura del lector), solo con
  Fortnite cerrado y solo ese archivo: nombre simple `.replay` dentro de la
  carpeta de repeticiones y el mismo tamaño que al leerlo. La decisión se
  guarda en `replays.reviewed` (migración v3) y no se vuelve a preguntar; si
  el archivo ya no está, se da por decidida. Borrar no cambia los totales.

### Paquete `career` (más estadísticas de la API)

La API da más contadores de los que usa el juego base: partidas, minutos
jugados, jugadores superados, muertes, `top3`/`top5`/`top6`/`top12`/`top25`
y cada modo por separado (solo, duo, squad, ltm). El paquete `career` los usa
sin necesitar repeticiones, también en consola: Veterano de la isla (100
partidas), Cazador (100 eliminaciones), Un día en la isla (24 h), Dúo dinámico,
Escuadrón de élite y Fuera de lo común (ganar en dúos, escuadrones y un modo
de tiempo limitado), Resistente (10 000 jugadores superados), Podio (25 top 3
en escuadrones), Masacre (15 eliminaciones en una partida) y Aterrizaje
forzoso (secreto: ser el primero en caer). Como paquete, no cuenta para el
platino. Ojo: en `all`, cada `top*` viene de un único modo (`top10`/`top25` de
solo, `top5`/`top12` de dúos, `top3`/`top6` de escuadrones).

## Pendiente de investigar

- Límites de peticiones de fortnite-api.com: no están documentados; se respeta `429`/`Retry-After`.
- Unidad de la distancia de las repeticiones y números de arma de `playerElim`,
  con repeticiones reales.
- **Descartado:** detectar cofres leyendo el teclado (cuánto se mantiene pulsada
  la tecla de interactuar). Es un hook global de teclado, prohibido por el
  ADR 0001 y típico de keyloggers. Además no es fiable: puertas, munición,
  reanimar, teclas reasignadas, mando y consola.
