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
- **Comportamiento:** se inicia con Windows, minimizada en la bandeja. Cerrar
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

## Pendiente de investigar

- Límites de peticiones de fortnite-api.com: no están documentados; se respeta `429`/`Retry-After`.
- Formato de los `.replay` de Fortnite (fase 2). En concreto, para los logros de
  cofres: si registran el momento en que cada cofre pasa a abierto y la posición
  del jugador, para atribuirle el cofre por proximidad. Solo sirve en PC con las
  repeticiones activadas; las partidas en consola no contarían.
- **Descartado:** detectar cofres leyendo el teclado (cuánto se mantiene pulsada
  la tecla de interactuar). Es un hook global de teclado, prohibido por el
  ADR 0001 y típico de keyloggers. Además no es fiable: puertas, munición,
  reanimar, teclas reasignadas, mando y consola.
- `tray-icon` junto con eframe en Windows (paso 6).
