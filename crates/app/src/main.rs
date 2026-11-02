fn main() {
    // Placeholder until the UI step; for now just proves the bundled data loads.
    let source = include_str!("../../../games/fortnite/achievements.toml");
    let fortnite = archi_core::GameDefinition::from_toml_str(source).expect("bundled Fortnite definition");
    println!("{}: {} achievements", fortnite.game.id, fortnite.achievements.len());
}
