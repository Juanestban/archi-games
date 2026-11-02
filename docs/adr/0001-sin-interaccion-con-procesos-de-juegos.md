# ADR 0001: Ninguna interacción con los procesos de los juegos

- Estado: aceptado
- Fecha: 2026-11-02

## Contexto

Archi Games convive con juegos protegidos por anti-cheat (Fortnite usa Easy
Anti-Cheat) y con antivirus. Un usuario no debe arriesgar un baneo por usar la
plataforma, y la app no debe parecer malware ni generar falsos positivos.

## Decisión

Reglas obligatorias. Cualquier función nueva que las rompa se rechaza.

**Frente al anti-cheat**

1. Nunca abrir un handle a un proceso de juego: nada de `OpenProcess`,
   lectura o escritura de memoria, inyección de DLLs, hooks de
   DirectX/Vulkan ni hooks globales de teclado o ratón.
2. Un juego se detecta solo por el **nombre** en la lista de procesos. Con
   `sysinfo`, refrescar únicamente nombres (sin memoria, rutas ni CPU), porque
   esas consultas abren handles.
3. Las notificaciones son una ventana propia de la app, nunca un overlay
   inyectado en el juego.
4. Los datos vienen de APIs públicas por HTTPS. Los archivos locales del juego
   (p. ej. `.replay`) solo se leen en modo lectura y cuando el juego ya los
   cerró.

**Frente a los antivirus**

5. Sin privilegios de administrador.
6. Inicio con el sistema solo mediante `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`:
   sin servicios ni tareas programadas ocultas.
7. Sin empaquetadores (UPX), ofuscación ni descarga y ejecución de código. La
   red solo va a las APIs configuradas.
8. Datos en `%APPDATA%\archi-games`. Se distribuye con un instalador MSI
   estándar y, a medio plazo, con el ejecutable firmado.

## Consecuencias

- No hay desbloqueos durante la partida: llegan al terminar, cuando la API se
  actualiza.
- Los logros solo pueden usar lo que exponen las APIs y, en la fase 2, las
  repeticiones.
