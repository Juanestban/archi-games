# Avisos de terceros

## Diseño de la interfaz

La interfaz parte del sistema visual de **ArtCraft**
(<https://github.com/storytold/artcraft>, MIT o Apache-2.0): superficies casi
negras, filos blancos al 15 %, esquinas de 3 px en los controles, barra de
título propia y barra lateral pegada al borde. `crates/app/src/ui/chrome.rs`
está adaptado de su `artcraft-native/src/ui/window.rs`.

## Fuentes

En `crates/app/assets/fonts/`, todas bajo la SIL Open Font License 1.1 (el
texto de cada licencia está junto a la fuente):

- **Inter**: © The Inter Project Authors.
- **Archivo**: © The Archivo Project Authors.
- **Geist Mono**: © The Geist Project Authors.

## Sonidos

Los sonidos se sintetizan en `crates/app/src/sound.rs`. Son originales: buscan
la sensación de un trofeo de consola sin reproducir ningún sonido existente.
