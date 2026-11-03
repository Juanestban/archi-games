# UI strings. Each game's own strings live in games/<id>/locales.

ui-nav-profile = Profile
ui-nav-games = Games
ui-nav-achievements = Achievements
ui-nav-history = History
ui-nav-settings = Settings

ui-tier-bronze = Bronze
ui-tier-silver = Silver
ui-tier-gold = Gold
ui-tier-platinum = Platinum

ui-level = Level { $level }
ui-xp-progress = { $current } / { $next } XP to level { $level }
ui-xp-total = { $xp } XP total
ui-account-none = Link a game account in Settings to get started.
ui-recent = Recent unlocks
ui-nothing-yet = No unlocks yet. Play a match: achievements are checked when it ends.

ui-completion = { $percent }% complete
ui-unlocked-count = { $unlocked } of { $total } achievements
ui-platinum-earned = Platinum earned
ui-view-achievements = View achievements

ui-filter-all = All
ui-filter-unlocked = Unlocked
ui-filter-locked = Locked
ui-filter-any-tier = Any tier
ui-search = Search achievements…
ui-no-matches = No achievements match these filters.
ui-hidden-name = Secret achievement
ui-hidden-desc = Keep playing to reveal it.
ui-pack = Add-on pack
ui-xp-reward = +{ $xp } XP
ui-unlocked-when = Unlocked { $when }

ui-settings-game = Game
ui-settings-language = Language
ui-language-system = System language
ui-settings-api-key-saved = Key saved
ui-settings-api-key-missing = No key
ui-save = Save
ui-link = Link
ui-linked-ok = Account linked. The first reading is your starting point.
ui-settings-tracking = Tracking
ui-game-running = { $game } is running: checking every minute.
ui-game-closed = { $game } is closed: checking every 12 minutes ({ $note }).
ui-last-poll = Last check: { $when }
ui-never = never
ui-poll-now = Check now
ui-busy = Checking…

ui-time-just-now = just now
ui-time-minutes = { $n } min ago
ui-time-hours = { $n } h ago
ui-time-days = { $n ->
    [one] { $n } day ago
   *[other] { $n } days ago
}
ui-time-date = on { $date }

ui-error-no-api-key = The API key is missing. Add it in Settings.
ui-error-unauthorized = The API key isn't valid.
ui-error-rate-limited = Too many requests; retrying in a few minutes.
ui-error-unexpected = The API sent something unexpected; will retry.
ui-error-storage = Couldn't read or write the database.
ui-error-credentials = Couldn't access Windows Credential Manager.

ui-toast-unlocked = Achievement unlocked
ui-toast-platinum = Platinum earned!
ui-toast-level-up = Level up!
ui-toast-sample = Sample achievement
ui-settings-sound = Sound and notifications
ui-settings-sound-help = Notifications show over the game in windowed or borderless fullscreen mode, not in exclusive fullscreen.
ui-volume = Volume
ui-notification-test = Test notification

ui-sidebar-library = Library
ui-status-tracking = Tracking
ui-status-in-game = In game
ui-status-setup = Not set up
ui-status-problem = Check Settings
ui-level-label = Level
ui-account-none-short = No account
ui-platinum-pending = Platinum pending

ui-tray-open = Open Archi Games
ui-tray-quit = Quit
ui-settings-system = System
ui-autostart = Start with Windows
ui-autostart-help = Opens in the tray when you sign in. Closing the window keeps it in the tray; to exit completely, use Quit in its icon's menu.
ui-error-autostart = Couldn't change start with Windows.

ui-update-available = Version { $version } is available
ui-update-notes = What's new
ui-update-install = Update
ui-update-retry = Retry
ui-update-downloading = Downloading version { $version }: { $percent }%
ui-update-installing = Installing the new version…
ui-settings-about = About
ui-version = Version { $version }
ui-license = Licensed under MIT or Apache-2.0
ui-update-auto = Check for updates automatically
ui-update-check = Check for updates
ui-update-checking = Checking for updates…
ui-update-latest = You're on the latest version.
ui-update-help = New versions come from the project's GitHub releases, are verified with SHA-256 and only install when you click Update.
ui-error-update = Couldn't check for or download the update.
ui-error-update-integrity = The download doesn't match its SHA-256 checksum; nothing was installed.

ui-replays-count = { $count ->
    [one] 1 replay read (Replays pack achievements).
   *[other] { $count } replays read (Replays pack achievements).
}
ui-replays-none = No replays read yet. For the Replays pack achievements, turn on Settings > Game > Record Replays in Fortnite (PC only).
ui-replays-review = { $count ->
    [one] Replay read ({ $size } MB). Delete it to free up space?
   *[other] { $count } replays read ({ $size } MB). Delete them to free up space?
}
ui-replays-review-wait = Close Fortnite to delete them.
ui-replays-delete = Delete
ui-replays-keep = Keep
ui-replays-delete-always = Always delete
ui-replays-auto-delete = Delete replays after reading them
ui-replays-auto-delete-help = Only the ones Archi Games has read, and only with Fortnite closed. You won't be able to watch them in Fortnite. Replays that can't be read are kept.
