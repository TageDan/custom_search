fn main() {
    let mut config_dir = std::env::home_dir().unwrap();
    config_dir.push(".config");
    config_dir.push("custom_search");
    if !std::path::Path::new(&config_dir).is_dir() {
        std::fs::create_dir_all(&config_dir).unwrap();
    }
    let mut config_file = config_dir;
    config_file.push("config.toml");
    if !std::path::Path::new(&config_file).exists() {
        std::fs::File::create(&config_file).unwrap();
    }
}
