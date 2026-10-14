fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}
