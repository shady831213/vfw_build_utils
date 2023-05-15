use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
pub extern crate cc;
pub extern crate handlebars;
extern crate serde;
use handlebars::Handlebars;
pub use v_build_utils::*;

pub fn c_src_dir<P: AsRef<Path> + Copy>(
    root_dir: P,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let mut c_files = vec![];
    let mut incdir = vec![];
    incdir.push(root_dir.as_ref().to_path_buf());
    println!("cargo:rerun-if-changed={}", root_dir.as_ref().display());
    walk_dir(root_dir, &mut |p: &PathBuf| {
        if p.is_file() {
            if let Some(ext) = p.extension() {
                if ext == OsStr::new("c") {
                    println!("cargo:rerun-if-changed={}", p.display());
                    c_files.push(p.clone());
                } else if ext == OsStr::new("h") {
                    println!("cargo:rerun-if-changed={}", p.display());
                }
            }
        } else if p.is_dir() {
            println!("cargo:rerun-if-changed={}", p.display());
            incdir.push(p.clone());
        }
        Ok(())
    })?;
    Ok((c_files, incdir))
}

pub fn build_c_files<'a, P: AsRef<Path> + Copy>(
    root_dir: P,
    build: &'a mut cc::Build,
) -> Result<Option<&'a mut cc::Build>, String> {
    let (c_files, incdir) = c_src_dir(root_dir)?;
    if c_files.len() > 0 {
        println!("cargo:rerun-if-changed={}", root_dir.as_ref().display());
        Ok(Some(
            build.files(&c_files).includes(&incdir).include(&root_dir),
        ))
    } else {
        Ok(None)
    }
}

pub fn tests_build(toolchain_prefix: &str) {
    tests_build_with(toolchain_prefix, |b| b)
}

pub fn tests_build_with<F: FnMut(&mut cc::Build) -> &mut cc::Build>(
    toolchain_prefix: &str,
    mut f: F,
) {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-env-changed=TESTNAME");
    let test_name = env::var("TESTNAME").unwrap();
    let test_dir = Path::new("src/bin").join(&test_name);
    let c_flags = test_dir.join("c_flags");
    println!("cargo:rerun-if-changed={}", c_flags.display());
    let mut c_build = cc::Build::new();
    if let Some(build) = build_c_files(&test_dir, &mut c_build).unwrap() {
        let b = build
            .compiler(format!("{}gcc", toolchain_prefix))
            .archiver(format!("{}ar", toolchain_prefix))
            .out_dir(out_dir)
            .flag("-Wno-main")
            .flag("-Wno-strict-aliasing")
            .flag("-Wno-builtin-declaration-mismatch");
        if let Ok(f) = fs::File::open(c_flags) {
            let reader = BufReader::new(f);
            for l in reader.lines() {
                let l = l.unwrap();
                let l = l.trim();
                if !l.is_empty() {
                    b.flag(l);
                }
            }
        }
        f(b).compile(&test_name);
    }
}

pub fn dep_header(dep: &str) -> Result<String, String> {
    dep_value(dep, "include")
}

pub struct HeaderDir(OtherDir);

impl HeaderDir {
    pub fn new() -> Result<Self, env::VarError> {
        Ok(HeaderDir(OtherDir::new("include")?))
    }
}
impl std::ops::Deref for HeaderDir {
    type Target = OtherDir;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct LinkFile<'a> {
    output: fs::File,
    te: Handlebars<'a>,
}

impl<'a> LinkFile<'a> {
    pub fn new(name: &str) -> Result<Self, String> {
        let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
        println!("cargo:rustc-link-search={}", out_dir.display());
        Ok(LinkFile {
            output: fs::File::create(out_dir.join(name)).map_err(|e| e.to_string())?,
            te: Handlebars::new(),
        })
    }
    pub fn add_file<P: AsRef<Path> + Copy>(&mut self, file: P) -> std::io::Result<&mut Self> {
        println!("cargo:rerun-if-changed={}", file.as_ref().display());
        let mut buffer = vec![];
        fs::File::open(file)?.read_to_end(&mut buffer)?;
        self.output.write_all(&buffer)?;
        Ok(self)
    }
    pub fn render_file<P: AsRef<Path> + Copy, T: serde::Serialize, F: FnOnce() -> T>(
        &mut self,
        file: P,
        f: F,
    ) -> Result<&mut Self, handlebars::RenderError> {
        println!("cargo:rerun-if-changed={}", file.as_ref().display());
        let mut buffer = String::new();
        fs::File::open(file)?.read_to_string(&mut buffer)?;
        self.te
            .render_template_to_write(&buffer, &f(), &mut self.output)?;
        Ok(self)
    }
}
