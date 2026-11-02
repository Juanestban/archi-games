# Archi Games

Plataforma de escritorio de logros estilo PlayStation para juegos que no tienen
sistema de logros, empezando por Fortnite. Logros propios, platino por juego,
nivel de cuenta y metalogros de la plataforma. Rust nativo con egui.

## Reglas inquebrantables

La app **nunca toca los procesos de los juegos**: no lee memoria, no inyecta
código y no hookea nada. Los datos llegan de APIs públicas. Detalles en
[ADR 0001](docs/adr/0001-sin-interaccion-con-procesos-de-juegos.md).

## Estructura

```
crates/core               motor de logros: definiciones, evaluación, XP, niveles,
                          trait GameTracker y cuándo consultar (Poller)
crates/storage            SQLite: lecturas, desbloqueos, cuentas y ajustes
crates/trackers/fortnite  cliente de fortnite-api.com
crates/app                la app: interfaz egui, textos (locales/) e hilo del tracker
games/<id>/   achievements.toml + locales/{es,en}.ftl de cada juego
docs/adr/     decisiones de arquitectura
```

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
```

`mode` es opcional (por defecto `all`, el total de todos los modos). Los
comparadores son `eq`, `gte` y `lte`. Otros campos: `tier`
(bronze/silver/gold/platinum), `hidden` y `pack`, para expansiones que no
cuentan para el platino. El platino no lleva condición: se desbloquea al
completar el juego base.

```
cargo test --workspace
```

## Probarlo con tu cuenta

Necesitas una clave gratuita de [fortnite-api.com](https://dash.fortnite-api.com)
y tener activado en Fortnite *Mostrar en la clasificación de carrera*.

```
cargo run
```

En **Ajustes**, guarda la clave (va al Administrador de credenciales de
Windows) y vincula tu nombre de Epic. La primera consulta es la base: el
progreso cuenta desde ahí. Juega una partida y, al terminarla, el logro aparece
en **Perfil** e **Historial**.

En Windows, `.cargo/config.toml` obliga a compilar las dependencias en C
(SQLite) con MSVC aunque el entorno tenga `CC=gcc`.
