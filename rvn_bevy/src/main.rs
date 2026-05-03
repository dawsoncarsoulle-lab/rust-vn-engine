fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./data".to_string());

    if let Err(e) = rvn_bevy::run_game_from_path(path) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
