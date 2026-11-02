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
ui-account-linked = Epic account: { $name }
ui-account-none = Link your Epic account in Settings to get started.
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
ui-no-matches = No achievements match these filters.
ui-hidden-name = Secret achievement
ui-hidden-desc = Keep playing to reveal it.
ui-pack = Add-on pack
ui-xp-reward = +{ $xp } XP
ui-unlocked-when = Unlocked { $when }

ui-settings-language = Language
ui-language-system = System language
ui-settings-api-key = fortnite-api.com API key
ui-settings-api-key-help = Free at dash.fortnite-api.com. Stored in Windows Credential Manager.
ui-settings-api-key-saved = Key saved
ui-settings-api-key-missing = No key
ui-save = Save
ui-settings-account = Epic account
ui-settings-epic-name = Epic name
ui-link = Link
ui-linked-ok = Account linked. The first reading is your starting point.
ui-settings-tracking = Tracking
ui-game-running = { $game } is running: checking every minute.
ui-game-closed = { $game } is closed: checking every 12 minutes (console matches count too).
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
ui-error-private = Your stats are private. In Fortnite: Settings > Account and Privacy > Show on Career Leaderboard.
ui-error-not-found = That account doesn't exist or has never played.
ui-error-unauthorized = The API key isn't valid.
ui-error-rate-limited = Too many requests; retrying in a few minutes.
ui-error-network = Can't reach fortnite-api.com; will retry.
ui-error-unexpected = The API sent something unexpected; will retry.
ui-error-storage = Couldn't read or write the database.
ui-error-credentials = Couldn't access Windows Credential Manager.
