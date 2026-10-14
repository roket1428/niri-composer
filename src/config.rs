use std::path::PathBuf;

pub fn validate(path: &Option<PathBuf>) {
    if let Some(location) = path {
        println!("given path: {:?}", location);
    } else {
        println!("no path specified");
    }

    // TODO: implement config validation
}
