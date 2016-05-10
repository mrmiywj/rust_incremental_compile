extern crate cargo;
extern crate git2;
extern crate walkdir;
extern crate libc;

use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io;
use std::os::unix::prelude::*;
use std::path::Path;
use std::env::args;

use cargo::core::{Source, SourceId, Registry, Dependency, PackageId};
use cargo::ops;
use cargo::sources::RegistrySource;
use cargo::core::shell::{Shell, MultiShell, Verbosity, ShellConfig, ColorConfig};
use cargo::util::Config;
use walkdir::{WalkDir, DirEntry, WalkDirIterator};

fn main() {
    if fs::metadata("index").is_err() {
        git2::Repository::clone("https://github.com/rust-lang/crates.io-index",
                                ".index").unwrap();
        fs::rename(".index", "index").unwrap();
    }

    let config = config();
    let id = SourceId::for_central(&config).unwrap();
    let mut s = RegistrySource::new(&id, &config);
    s.update().unwrap();

    let stdout = unsafe { libc::dup(1) };
    let stderr = unsafe { libc::dup(2) };
    assert!(stdout > 0 && stderr > 0);

    let root = env::current_dir().unwrap();
    let mut arg = env::args();
    arg.next();
    let c = arg.next().unwrap();
    println!("crate: {}", c);
    let latest_version = arg.next().unwrap();
    println!("latestL {}", latest_version);
    for v in arg {
        build(&root.join("incremental"), &mut s, &id, c.as_str(), v.as_str());
    }
    build(&root.join("noincremental"), &mut s, &id, c.as_str(), latest_version.as_str());
}


fn config() -> Config {
    let config = ShellConfig {
        color_config: ColorConfig::Always,
        tty: true,
    };
    let out = Shell::create(Box::new(io::stdout()), config);
    let err = Shell::create(Box::new(io::stderr()), config);
    Config::new(MultiShell::new(out, err, Verbosity::Verbose),
                env::current_dir().unwrap(),
                env::home_dir().unwrap()).unwrap()
}

fn build(out: &Path, src: &mut RegistrySource, id: &SourceId, krate: &str, ver: &str) {
    println!("working on: {}", krate);
    fs::create_dir_all(&out).unwrap();
    unsafe {
        let stdout = File::create(out.join("stdio")).unwrap();
        assert_eq!(libc::dup2(stdout.as_raw_fd(), 1), 1);
        assert_eq!(libc::dup2(stdout.as_raw_fd(), 2), 2);
    }
    let pkg = PackageId::new(krate, ver, id).unwrap();
    let pkg = match src.download(&pkg) {
        Ok(v) => v,
        Err(e) => {
            return println!("bad get pkg: {}: {}", pkg, e);
        }
    };

    fs::create_dir_all(".cargo").unwrap();
    File::create(".cargo/config").unwrap().write_all(format!("
        [build]
        target-dir = '{}'
    ", out.join("target").display()).as_bytes()).unwrap();

    let config = config();
    let args = &["-Z".to_string(), "incremental=temp/".to_string()];
    let res = ops::compile_pkg(&pkg, None, &ops::CompileOptions {
        config: &config,
        jobs: None,
        target: None,
        features: &[],
        no_default_features: false,
        spec: &[],
        filter: ops::CompileFilter::Only {
            lib: true,
            examples: &[],
            bins: &[],
            tests: &[],
            benches: &[],
        },
        exec_engine: None,
        release: true,
        mode: ops::CompileMode::Build,
        target_rustc_args: Some(args),
        target_rustdoc_args: None,
    });
    fs::remove_file(".cargo/config").unwrap();
    if let Err(e) = res {
        println!("bad compile {}: {}", pkg, e);
    } else {
        println!("OK");
    }
}
