fn cleanup(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)?;
    Ok(())
}
