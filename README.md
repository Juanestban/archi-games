<div align="center">

<img src="docs/assets/icon.png" alt="Icono de Archi Games" width="96">

# Archi Games

**Logros estilo PlayStation para juegos que no los tienen: Fortnite,
Counter-Strike 2 y League of Legends. Rust nativo con egui.**

[![CI](https://github.com/Juanestban/archi-games/actions/workflows/ci.yml/badge.svg)](https://github.com/Juanestban/archi-games/actions/workflows/ci.yml)
[![Licencia: MIT OR Apache-2.0](https://img.shields.io/badge/licencia-MIT%20OR%20Apache--2.0-blue.svg)](#licencia)
![Rust](https://img.shields.io/badge/rust-edici%C3%B3n%202024-orange.svg)
![Plataforma](https://img.shields.io/badge/plataforma-Windows-0078D6.svg)

</div>

Plataforma de escritorio de logros para juegos que no tienen sistema de logros
(o con logros propios distintos). Logros propios, platino por juego y por
paquete, nivel de cuenta y metalogros de la plataforma.

> [!NOTE]
> Archi Games funciona en **Windows**. League of Legends todavía no se ha
> probado con una partida real. El estado de cada parte está en
> [ROADMAP.md](ROADMAP.md).

## Reglas inquebrantables

La app **nunca toca los procesos de los juegos**: no lee memoria, no inyecta
código y no hookea nada. Los datos llegan de APIs públicas, de las repeticiones
de Fortnite, de lo que el propio CS2 envía con su integración oficial y de los
datos de la partida que League of Legends ofrece en el propio equipo. Detalles en
[ADR 0001](docs/adr/0001-sin-interaccion-con-procesos-de-juegos.md).

## Instalación

Cada [release](https://github.com/Juanestban/archi-games/releases) trae dos
descargas para Windows:

- **`ArchiGames-<versión>-x64.msi`**: se instala solo para tu usuario en
  `%LOCALAPPDATA%\Programs\Archi Games`, sin permisos de administrador. Se
  desinstala desde *Configuración → Aplicaciones*.
- **`ArchiGames-<versión>-x64-portable.exe`**: un único archivo. Cópialo donde
  quieras y ábrelo.

Las dos avisan de las versiones nuevas y se actualizan solas al pulsar
**Actualizar** ([ADR 0002](docs/adr/0002-actualizaciones-desde-github.md)).

No están firmadas (como las de muchos proyectos de código abierto), así que
Windows SmartScreen puede mostrar *"Windows protegió su PC"* o *"editor
desconocido"*. Para abrirlas de todas formas: **Más información → Ejecutar de
todas formas**. Para comprobar que el archivo es el publicado, compara su
SHA-256 (`Get-FileHash <archivo>`) con `SHA256SUMS.txt` de la misma release.

También puedes compilarla desde el código (ver
[Compilar desde el código](#compilar-desde-el-código)).

## Primeros pasos

En **Ajustes**, elige el juego, guarda su clave si la necesita (va al
Administrador de credenciales de Windows) y vincula tu cuenta. La primera
consulta es la base: el progreso cuenta desde ahí. Juega una partida y, al
terminarla, el logro aparece en **Perfil** e **Historial**.

### Fortnite

1. Consigue una clave gratuita en [fortnite-api.com](https://dash.fortnite-api.com).
2. En Fortnite, activa *Mostrar en la clasificación de carrera* para que tus
   estadísticas sean públicas.
3. En Archi Games, **Ajustes > Fortnite**: pega la clave y vincula tu nombre de
   Epic.

#### Repeticiones (paquete "Repeticiones", solo PC)

Algunos logros (precisión, daño recibido, materiales, distancia…) salen de las
repeticiones que Fortnite guarda en el PC. Para que cuenten:

1. En Fortnite, abre **Ajustes > Juego** y activa **Grabar repeticiones**.
2. Juega como siempre. Fortnite guarda cada partida en
   `%LOCALAPPDATA%\FortniteGame\Saved\Demos`.
3. Cierra Fortnite. Archi Games lee las repeticiones nuevas unos segundos
   después (y cada 2 minutos con el juego cerrado), solo en modo lectura.

Solo cuentan las partidas posteriores a vincular la cuenta, y las de consola no
generan repeticiones. Por eso este paquete no cuenta para el platino. En
**Ajustes > Seguimiento** puedes hacer que la app borre las repeticiones que ya
leyó, para ahorrar espacio.

### Counter-Strike 2

Sin clave. En **Ajustes > Counter-Strike 2** pulsa **Activar**: se copia
`gamestate_integration_archigames.cfg` en la carpeta `cfg` de CS2 y la app
escucha en `127.0.0.1:43117`. Abre (o reinicia) CS2 y juega: la cuenta se
vincula sola y cada partida cuenta al terminar.

### League of Legends

Nada que configurar. Durante cada partida la app lee
`https://127.0.0.1:2999/liveclientdata/allgamedata`, que sirve el propio juego;
la cuenta (tu Riot ID) se vincula sola y la partida cuenta al acabar. No
cuentan la herramienta de práctica, las partidas con bots ni los remakes (menos
de 5 minutos).

## Compilar desde el código

### Requisitos (todas las plataformas)

- [Rust](https://rustup.rs/) estable (la edición 2024 necesita 1.85 o más nuevo)
- Git
- Un driver de GPU con DirectX 12, Vulkan, Metal u OpenGL (lo usa egui)

### Windows (plataforma principal)

1. Instala las **Visual Studio Build Tools** con la carga de trabajo
   *Desarrollo para el escritorio con C++*. Rust las necesita para enlazar, y
   el Windows SDK que incluyen trae `rc.exe`, que incrusta el icono en el `.exe`.
2. Instala Rust desde <https://rustup.rs/> (la toolchain por defecto,
   `x86_64-pc-windows-msvc`).
3. Clona y compila:

   ```powershell
   git clone https://github.com/Juanestban/archi-games.git
   cd archi-games
   cargo build --release -p archi-games
   ```

4. El ejecutable portable queda en `target\release\archi-games.exe`.

`.cargo/config.toml` obliga a compilar las dependencias en C (SQLite) con MSVC
aunque el entorno tenga `CC=gcc`.

### macOS (sin soporte oficial todavía)

El código compila para otras plataformas, pero no está probado y aún falta lo
propio de cada sistema: la detección de los juegos, el inicio con el sistema y
la instancia única solo existen en Windows.

```sh
xcode-select --install                       # compilador y enlazador
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/Juanestban/archi-games.git
cd archi-games
cargo build --release -p archi-games
./target/release/archi-games
```

### Linux (sin soporte oficial todavía)

Igual que en macOS. La interfaz, la bandeja, el sonido y el llavero del sistema
necesitan algunos paquetes de desarrollo. En Debian/Ubuntu:

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev \
  libasound2-dev libdbus-1-dev libssl-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/Juanestban/archi-games.git
cd archi-games
cargo build --release -p archi-games
./target/release/archi-games
```

En Fedora, los equivalentes: `gcc pkg-config libxkbcommon-devel libxcb-devel
gtk3-devel libxdo-devel libayatana-appindicator-gtk3-devel alsa-lib-devel
dbus-devel openssl-devel`.

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

## Definir logros

Los textos son tokens [Fluent](https://projectfluent.org/) traducidos en
`locales/{es,en}.ftl`. Hay cuatro tipos de condición:

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

## Desarrollo

```
cargo run                                               # abrir la app
cargo test --workspace                                  # tests
cargo fmt --all --check                                 # formato
cargo clippy --workspace --all-targets -- -D warnings   # lint
```

Para comprobar el lector de repeticiones con archivos reales (copias):

```
$env:REPLAY_DIR = "..."; $env:ACCOUNT_ID = "<id de Epic>"
cargo test -p archi-fortnite -- --ignored --nocapture
```

Para comprobar League of Legends con una partida en curso:
`cargo test -p archi-lol reads_a_live_match -- --ignored --nocapture`.

Para publicar una versión, ver [RELEASING.md](RELEASING.md). Para empaquetar
en local:

```
dotnet tool restore              # WiX 5, local al repositorio
.\scripts\package.ps1            # -> target\dist (MSI, portable y SHA256SUMS.txt)
```

El icono de `docs/assets/icon.png` es una captura del que dibuja
`crates/app/src/icon.rs`; si cambia el dibujo, hay que regenerarlo.

## Licencia

A tu elección, [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE), igual que
ArtCraft. Salvo que digas lo contrario, cualquier contribución se publica bajo
esas mismas dos licencias.

## Créditos

La interfaz parte del diseño de [ArtCraft](https://github.com/storytold/artcraft)
y usa las fuentes Inter, Archivo y Geist Mono (OFL). Detalles en
[NOTICE.md](NOTICE.md).
