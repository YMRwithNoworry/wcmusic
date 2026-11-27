fn main() {
    let manifest = "../../pubspec.yaml";
    println!("cargo:rerun-if-changed={manifest}");
    let pubspec = std::fs::read_to_string(manifest).expect("读取 pubspec.yaml 失败");
    let version = pubspec
        .lines()
        .find_map(|line| line.strip_prefix("version: "))
        .and_then(|value| value.split_once('+'))
        .map(|(version, _)| version)
        .expect("pubspec.yaml 缺少发布版本");
    println!("cargo:rustc-env=WCMUSIC_VERSION={version}");
}
