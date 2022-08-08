use std::path::PathBuf;

pub fn get_absolute_path(path: &str) -> String {
   let relative_path = PathBuf::from(path);
   let mut absolute_path = std::env::current_dir().unwrap();
   absolute_path.push(relative_path);
   absolute_path.display().to_string()
}