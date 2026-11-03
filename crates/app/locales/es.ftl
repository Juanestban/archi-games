# Textos de la interfaz. Los de cada juego están en games/<id>/locales.

ui-nav-profile = Perfil
ui-nav-games = Juegos
ui-nav-achievements = Logros
ui-nav-history = Historial
ui-nav-settings = Ajustes

ui-tier-bronze = Bronce
ui-tier-silver = Plata
ui-tier-gold = Oro
ui-tier-platinum = Platino

ui-level = Nivel { $level }
ui-xp-progress = { $current } / { $next } XP para el nivel { $level }
ui-xp-total = { $xp } XP en total
ui-account-linked = Cuenta de Epic: { $name }
ui-account-none = Vincula tu cuenta de Epic en Ajustes para empezar.
ui-recent = Desbloqueos recientes
ui-nothing-yet = Todavía no hay desbloqueos. Juega una partida: se comprueban al terminarla.

ui-completion = { $percent } % completado
ui-unlocked-count = { $unlocked } de { $total } logros
ui-platinum-earned = Platino conseguido
ui-view-achievements = Ver logros

ui-filter-all = Todos
ui-filter-unlocked = Desbloqueados
ui-filter-locked = Pendientes
ui-filter-any-tier = Cualquier rango
ui-no-matches = Ningún logro coincide con los filtros.
ui-hidden-name = Logro secreto
ui-hidden-desc = Sigue jugando para descubrirlo.
ui-pack = Paquete adicional
ui-xp-reward = +{ $xp } XP
ui-unlocked-when = Desbloqueado { $when }

ui-settings-language = Idioma
ui-language-system = Idioma del sistema
ui-settings-api-key = Clave de API de fortnite-api.com
ui-settings-api-key-help = Es gratis en dash.fortnite-api.com. Se guarda en el Administrador de credenciales de Windows.
ui-settings-api-key-saved = Clave guardada
ui-settings-api-key-missing = Sin clave
ui-save = Guardar
ui-settings-account = Cuenta de Epic
ui-settings-epic-name = Nombre de Epic
ui-link = Vincular
ui-linked-ok = Cuenta vinculada. La primera lectura es tu punto de partida.
ui-settings-tracking = Seguimiento
ui-game-running = { $game } abierto: se consulta cada minuto.
ui-game-closed = { $game } cerrado: se consulta cada 12 minutos (también cuentan las partidas en consola).
ui-last-poll = Última consulta: { $when }
ui-never = nunca
ui-poll-now = Consultar ahora
ui-busy = Consultando…

ui-time-just-now = ahora mismo
ui-time-minutes = hace { $n } min
ui-time-hours = hace { $n } h
ui-time-days = hace { $n ->
    [one] { $n } día
   *[other] { $n } días
}
ui-time-date = el { $date }

ui-error-no-api-key = Falta la clave de API. Añádela en Ajustes.
ui-error-private = Tus estadísticas son privadas. En Fortnite: Ajustes > Cuenta y privacidad > Mostrar en la clasificación de carrera.
ui-error-not-found = Esa cuenta no existe o nunca ha jugado.
ui-error-unauthorized = La clave de API no es válida.
ui-error-rate-limited = Demasiadas consultas; se reintentará en unos minutos.
ui-error-network = No hay conexión con fortnite-api.com; se reintentará.
ui-error-unexpected = La API respondió algo inesperado; se reintentará.
ui-error-storage = No se pudo leer o escribir la base de datos.
ui-error-credentials = No se pudo acceder al Administrador de credenciales de Windows.

ui-toast-unlocked = Logro desbloqueado
ui-toast-platinum = ¡Platino conseguido!
ui-toast-level-up = ¡Subes de nivel!
ui-toast-sample = Logro de ejemplo
ui-settings-sound = Sonido y notificaciones
ui-settings-sound-help = La notificación aparece sobre el juego en modo ventana o pantalla completa sin bordes, no en pantalla completa exclusiva.
ui-volume = Volumen
ui-notification-test = Probar notificación

ui-sidebar-library = Biblioteca
ui-status-tracking = Siguiendo
ui-status-in-game = En partida
ui-status-setup = Sin configurar
ui-status-problem = Revisa Ajustes
ui-level-label = Nivel
ui-account-none-short = Sin cuenta
ui-platinum-pending = Platino pendiente

ui-tray-open = Abrir Archi Games
ui-tray-quit = Salir
ui-settings-system = Sistema
ui-autostart = Iniciar con Windows
ui-autostart-help = Se abre en la bandeja al iniciar sesión. Cerrar la ventana la deja en la bandeja; para salir del todo, usa Salir en el menú de su icono.
ui-error-autostart = No se pudo cambiar el inicio con Windows.

ui-update-available = Nueva versión { $version } disponible
ui-update-notes = Novedades
ui-update-install = Actualizar
ui-update-retry = Reintentar
ui-update-downloading = Descargando la versión { $version }: { $percent } %
ui-update-installing = Instalando la nueva versión…
ui-settings-about = Acerca de
ui-version = Versión { $version }
ui-license = Licencia MIT o Apache-2.0
ui-update-auto = Buscar actualizaciones automáticamente
ui-update-check = Buscar actualizaciones
ui-update-checking = Buscando actualizaciones…
ui-update-latest = Tienes la última versión.
ui-update-help = Las versiones nuevas vienen de las releases del proyecto en GitHub, se verifican con SHA-256 y solo se instalan cuando pulsas Actualizar.
ui-error-update = No se pudo comprobar o descargar la actualización.
ui-error-update-integrity = La descarga no coincide con su suma SHA-256; no se ha instalado nada.

ui-replays-count = { $count ->
    [one] 1 repetición analizada (logros del paquete Repeticiones).
   *[other] { $count } repeticiones analizadas (logros del paquete Repeticiones).
}
ui-replays-none = Ninguna repetición analizada todavía. Para los logros del paquete Repeticiones, activa en Fortnite Ajustes > Juego > Grabar repeticiones (solo PC).
ui-replays-review = { $count ->
    [one] Repetición analizada ({ $size } MB). ¿Borrarla para liberar espacio?
   *[other] { $count } repeticiones analizadas ({ $size } MB). ¿Borrarlas para liberar espacio?
}
ui-replays-review-wait = Cierra Fortnite para poder borrarlas.
ui-replays-delete = Borrar
ui-replays-keep = Conservar
ui-replays-delete-always = Borrar siempre
ui-replays-auto-delete = Borrar las repeticiones después de analizarlas
ui-replays-auto-delete-help = Solo las que Archi Games ya leyó, y con Fortnite cerrado. Ya no podrás verlas en Fortnite. Las que no se pueden leer se conservan.
