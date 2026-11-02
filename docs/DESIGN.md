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

- Endpoints, límites y formato exacto de fortnite-api.com (paso 3).
- Formato de los `.replay` de Fortnite (fase 2).
- `tray-icon` junto con eframe en Windows (paso 6).
