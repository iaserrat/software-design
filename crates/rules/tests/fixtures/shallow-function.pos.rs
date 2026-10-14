fn add_null_value(attrs: &mut HashMap<String, Option<String>>, name: &str) { attrs.insert(name.to_owned(), None); }
pub fn reset(attrs: &mut HashMap<String, Option<String>>) { add_null_value(attrs, "id") }
