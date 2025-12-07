use std::path::PathBuf;
use std::process;

use crate::utils::hash::hash_file;

pub fn run(file1: PathBuf, file2: PathBuf) {
    let hash1 = match hash_file(&file1) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Error reading {}: {}", file1.display(), e);
            process::exit(2);
        }
    };

    let hash2 = match hash_file(&file2) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Error reading {}: {}", file2.display(), e);
            process::exit(2);
        }
    };

    println!("{}: {}", file1.display(), hash1);
    println!("{}: {}", file2.display(), hash2);

    if hash1 == hash2 {
        println!("Files match");
        process::exit(0);
    } else {
        println!("Files differ");
        process::exit(1);
    }
}
