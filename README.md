# Archi Games

Plataforma de escritorio de logros estilo PlayStation para juegos que no tienen
sistema de logros (o con logros propios distintos): Fortnite, Counter-Strike 2 y
League of Legends. Logros propios, platino por juego y por paquete,
nivel de cuenta y metalogros de la plataforma. Rust nativo con egui.

## Reglas inquebrantables

La app **nunca toca los procesos de los juegos**: no lee memoria, no inyecta
código y no hookea nada. Los datos llegan de APIs públicas, de las repeticiones
de Fortnite, de lo que el propio CS2 envía con su integración oficial y de los
datos de la partida que League of Legends ofrece en el propio equipo. Detalles en
[ADR 0001](docs/adr/0001-sin-interaccion-con-procesos-de-juegos.md).

## Estructura

```
crates/core               motor de logros: definiciones, evaluación, XP, niveles,
                          trait GameTracker y cuándo consultar (Poller)
crates/storage            SQLite: lecturas, desbloqueos, cuentas y ajustes
crates/trackers/fortnite  cliente de fortnite-api.com y lector de repeticiones
crates/trackers/cs2       Game State Integration de CS2: datos de la partida y el .cfg
crates/trackers/lol       Live Client Data API de League of Legends: partidas terminadas
crates/app                la app: interfaz egui, textos (locales/) e hilo del tracker
games/<id>/   achievements.toml + locales/{es,en}.ftl de cada juego
docs/adr/     decisiones de arquitectura
```

## Licencia

A tu elección, [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE), igual que
ArtCraft. Salvo que digas lo contrario, cualquier contribución se publica bajo
esas mismas dos licencias.

## Créditos

La interfaz parte del diseño de [ArtCraft](https://github.com/storytold/artcraft)
y usa las fuentes Inter, Archivo y Geist Mono (OFL). Detalles en
[NOTICE.md](NOTICE.md).

## Definir logros

Los textos son tokens [Fluent](https://projectfluent.org/) traducidos en
`locales/{es,en}.ftl`. Hay tres tipos de condición:

```toml
# Acumulado desde que se vinculó la cuenta
condition = { type = "total", stat = "wins", target = 100 }

# Dentro de una sola partida (solo se evalúa si entre dos lecturas hubo exactamente 1)
condition = { type = "match", mode = "solo", all = [{ stat = "wins", eq = 1 }, { stat = "kills", gte = 10 }] }

# N partidas seguidas que cumplen `all`
condition = { type = "streak", all = [{ stat = "wins", eq = 1 }], count = 3 }

# El valor actual, no lo sumado: para lo que empieza de cero cada temporada
condition = { type = "reach", mode = "battlepass", stat = "level", target = 100 }
```

`mode` es opcional (por defecto `all`, el total de todos los modos). En
`[game]`, `local_modes` lista los modos que la app suma ella misma a partir de
partidas locales (repeticiones, Game State Integration): empiezan en 0. En los
demás, una estadística que aparece después de la vinculación (porque una versión
nueva la lee) cuenta desde ese momento. Los
comparadores son `eq`, `gte` y `lte`. Otros campos: `tier`
(bronze/silver/gold/platinum), `hidden` y `pack` (paquete; su nombre es el
token `<juego>-pack-<pack>`). Un platino no lleva condición: el del juego base
llega al completar el juego base, y el de un paquete (un platino con `pack`) al
completar ese paquete. Cada parte tiene como mucho uno; un paquete puede no
tener ninguno.

```
cargo test --workspace
```

## Descargar e instalar

En [Releases](https://github.com/Juanestban/archi-games/releases) está cada
versión como instalador MSI (por usuario, sin administrador; se instala en
`%LOCALAPPDATA%\Programs\Archi Games`) y como `.exe` portable. La app avisa de
las versiones nuevas y se actualiza sola al pulsar **Actualizar**
([ADR 0002](docs/adr/0002-actualizaciones-desde-github.md)).

El instalador y el `.exe` no están firmados (como los de muchos proyectos de
código abierto), así que Windows SmartScreen puede mostrar *"Windows protegió
su PC"* o *"editor desconocido"*. Para instalarlo de todas formas: **Más
información → Ejecutar de todas formas**. Si quieres comprobar que el archivo
es el publicado, compara su SHA-256 (`Get-FileHash <archivo>`) con
`SHA256SUMS.txt` de la misma release.

Para publicar una versión, ver [RELEASING.md](RELEASING.md). Para empaquetar
en local:

```
dotnet tool restore              # WiX 5, local al repositorio
.\scripts\package.ps1            # -> target\dist (MSI, portable y SHA256SUMS.txt)
```

## Probarlo con tu cuenta

```
cargo run
```

En **Ajustes**, elige el juego, guarda su clave (va al Administrador de
credenciales de Windows) y vincula tu cuenta. La primera consulta es la base:
el progreso cuenta desde ahí. Juega una partida y, al terminarla, el logro
aparece en **Perfil** e **Historial**.

- **Fortnite:** clave gratuita de [fortnite-api.com](https://dash.fortnite-api.com),
  tu nombre de Epic y, en Fortnite, *Mostrar en la clasificación de carrera*.
- **Counter-Strike 2:** sin clave. En Ajustes > Counter-Strike 2 pulsa
  **Activar**: se copia `gamestate_integration_archigames.cfg` en la carpeta
  `cfg` de CS2 y la app escucha en `127.0.0.1:43117`. Abre (o reinicia) CS2 y
  juega: la cuenta se vincula sola y cada partida cuenta al terminar.
- **League of Legends:** nada que configurar. Durante cada partida la app lee
  `https://127.0.0.1:2999/liveclientdata/allgamedata`, que sirve el propio
  juego; la cuenta (tu Riot ID) se vincula sola y la partida cuenta al acabar.
  No cuentan la herramienta de práctica, las partidas con bots ni los remakes
  (menos de 5 minutos). Para comprobarlo con una partida en curso:
  `cargo test -p archi-lol reads_a_live_match -- --ignored --nocapture`.

Para comprobar el lector de repeticiones con archivos reales (copias):

```
$env:REPLAY_DIR = "..."; $env:ACCOUNT_ID = "<id de Epic>"
cargo test -p archi-fortnite -- --ignored --nocapture
```

En Windows, `.cargo/config.toml` obliga a compilar las dependencias en C
(SQLite) con MSVC aunque el entorno tenga `CC=gcc`.
