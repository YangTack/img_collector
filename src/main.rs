use opencv::{
    core::{CV_8U, Vector},
    highgui::{imshow, wait_key},
    imgcodecs::imwrite,
    prelude::*,
    videoio::{CAP_PROP_FRAME_HEIGHT, CAP_PROP_FRAME_WIDTH, VideoCapture},
};

use clap::Parser;
use std::{boxed::Box, fmt::Debug};
use std::{collections::HashMap, fs};
use std::{error::Error, path::PathBuf};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
};
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    file: String,

    #[arg(short, long, default_value = "data")]
    store_path: String,
}

trait VideoSize {
    fn width(&self) -> Result<i32, Box<dyn Error>>;
    fn height(&self) -> Result<i32, Box<dyn Error>>;
}

impl VideoSize for VideoCapture {
    fn width(&self) -> Result<i32, Box<dyn Error>> {
        Ok(self.get(CAP_PROP_FRAME_WIDTH)?.round() as i32)
    }

    fn height(&self) -> Result<i32, Box<dyn Error>> {
        Ok(self.get(CAP_PROP_FRAME_HEIGHT)?.round() as i32)
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Path error: {0}")]
    PathError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Glob error: {0}")]
    GlobError(#[from] glob::GlobError),

    #[error("GlobPattern error: {0}")]
    GlobPatternError(#[from] glob::PatternError),
}

trait FileIndice {
    fn from_data_path(path: &str) -> Result<Self, AppError>
    where
        Self: Sized;
}

trait FileIndiceHashMapAllowTypes {}
impl FileIndiceHashMapAllowTypes for i32 {}
impl FileIndiceHashMapAllowTypes for i64 {}

impl<T> FileIndice for HashMap<String, T>
where
    T: FileIndiceHashMapAllowTypes + From<i32> + std::ops::AddAssign + Clone + ToString,
{
    fn from_data_path(path: &str) -> Result<Self, AppError> {
        let mut result = Self::with_capacity(100);
        let base_path = PathBuf::from(path).canonicalize()?;
        for entry in glob::glob(&base_path.join("**/*.png").to_string_lossy())? {
            let entry = entry?;
            let parent = entry
                .parent()
                .ok_or(AppError::PathError("Missing parent directory".into()))?;
            let parent_str = parent
                .to_str()
                .ok_or(AppError::PathError("Invalid UTF-8 path".into()))?;
            let count = result.entry(parent_str.into()).or_insert(0.into());
            let new_name = format!("{}.png_", count.to_string());
            let _ = fs::rename(&entry, parent.join(new_name));
            *count += 1.into();
        }
        let path = PathBuf::from(path);
        for entry in glob::glob(&path.join("**/*.png_").to_string_lossy())? {
            let entry = entry?;
            let parent = entry
                .parent()
                .ok_or(AppError::PathError("Missing parent dir".into()))?;

            let name = entry
                .file_name()
                .ok_or(AppError::PathError("File name cannot find".into()))?
                .to_string_lossy();
            let name = name
                .get(0..name.len() - 1)
                .ok_or(AppError::PathError("Name length is too short".into()))?;
            let _ = fs::rename(&entry, &parent.join(name));
        }

        Ok(result)
    }
}

fn create_data_dir(path: &str) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut video = VideoCapture::from_file_def(&args.file)?;
    let width = video.width()?;
    let height = video.height()?;
    let _ = create_data_dir(&args.store_path);
    let mut indice_map = HashMap::<String, i32>::from_data_path(&args.store_path)?;
    println!("{:?}", indice_map);

    let mut store_img = unsafe { Mat::new_size((height, width).into(), CV_8U)? };
    let compression_params = Vector::<i32>::new();
    loop {
        if let Ok(true) = video.read(&mut store_img) {}
        if imshow("video", &store_img).is_err() {
            break;
        }

        if let Some(key) = wait_key(1).ok() {
            if key == -1 {
                continue;
            }
            let key = match char::from_u32(key as u32) {
                Some(k) => k,
                None => continue,
            };
            let dir = PathBuf::from(&args.store_path)
                .join(key.to_string())
                .canonicalize()?;
            let _ = create_data_dir(dir.to_str().unwrap());
            let index = indice_map
                .entry(dir.to_str().unwrap().to_string())
                .or_insert(0);
            let path = dir.join(format!("{}.png", index));
            println!("save img to {:?}", path);
            *index += 1;
            let compression_params_clone = compression_params.clone();
            let _ = imwrite(
                path.to_str()
                    .ok_or(AppError::PathError("pathbuf to_str err".into()))?,
                &store_img,
                &compression_params_clone,
            );
        }
    }
    let _ = video.release();
    Ok(())
}
