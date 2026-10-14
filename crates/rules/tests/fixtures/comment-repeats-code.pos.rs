fn build(panel: &mut Panel) {
    // Add a horizontal scroll bar
    let h_scroll_bar = ScrollBar::new(ScrollBar::HORIZONTAL);
    panel.add(h_scroll_bar);
}
/// Downcast the parameter to the type.
pub fn down_cast_parameter(parameter: &str, r#type: &str) -> Value { convert(parameter, r#type) }
