use std::path::PathBuf;

pub async fn daemon(config: &Option<PathBuf>) -> Result<(), std::io::Error> {
    println!("daemon will run!");
    if let Some(path) = config {
        println!("given path {:?}", path);
    }

    //
    // TODO: implement async tasks
    //

    Ok(())
}
