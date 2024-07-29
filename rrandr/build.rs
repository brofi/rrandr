use core::panic;
use std::env::var;
use std::fs::{self, create_dir_all, File};
use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::Command;

use config::Config;

fn main() {
    glib_build_tools::compile_resources(
        &["src/res"],
        "src/res/rrandr.gresource.xml",
        "rrandr.gresource",
    );
    println!("cargo::rustc-env=RRANDR_COPYRIGHT_NOTICE={}", copyright_notice());
    println!(
        "cargo::rustc-env=RRANDR_LOCALE_DIR={}",
        if cfg!(debug_assertions) {
            Path::new(&var("OUT_DIR").unwrap()).join("po").to_str().unwrap().to_owned()
        } else {
            var("LOCALEDIR").unwrap_or("/usr/share/locale".to_owned())
        }
    );
    gen_translations();
    gen_config();
}

fn copyright_notice() -> String {
    let from = 2024;
    let to = time::OffsetDateTime::now_utc().year();
    let years = if to > from { format!("{}-{}", from, to) } else { from.to_string() };
    format!("Copyright (C) {} {}.", years, copyright_holder())
}

fn copyright_holder() -> String {
    let authors = env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>();
    authors[0].split("<").collect::<Vec<_>>()[0].trim().to_owned()
}

fn gen_translations() {
    let pkg_name = env!("CARGO_PKG_NAME");
    let out = Path::new("po").join(env!("CARGO_PKG_NAME").to_owned() + ".pot");

    // Generate PO template file
    let mut gen_pot_rs = Command::new(home::cargo_home().expect("cargo home").join("bin/xtr"));
    gen_pot_rs.arg("--output").arg(&out).arg("--omit-header").arg("src/main.rs");
    check_cmd(&mut gen_pot_rs);
    let mut gen_pot_ui = Command::new("xgettext");
    gen_pot_ui
        .arg("--files-from=po/POTFILES.in")
        .arg(format!("--output={}", out.to_str().unwrap()))
        .args(["--join-existing", "--sort-by-file"])
        .args(["--copyright-holder", &copyright_holder()])
        .args(["--package-name", pkg_name])
        .args(["--package-version", env!("CARGO_PKG_VERSION")])
        .args(["--msgid-bugs-address", &(env!("CARGO_PKG_REPOSITORY").to_owned() + "/issues")]);
    check_cmd(&mut gen_pot_ui);

    // Compile POs
    let mut linguas = File::open("po/LINGUAS").expect("LINGUAS file exists");
    let mut buf = String::new();
    linguas.read_to_string(&mut buf).expect("LINGUAS file is valid UTF-8");
    for lang in buf.lines() {
        println!("cargo::rerun-if-changed=po/{lang}.po");
        let out_dir = Path::new(&var("OUT_DIR").unwrap()).join("po").join(lang).join("LC_MESSAGES");
        create_dir_all(&out_dir).expect("create output directories");
        let mut compile_po = Command::new("msgfmt");
        compile_po
            .arg(format!("--output-file={}/{}.mo", out_dir.to_str().unwrap(), pkg_name))
            .arg(format!("po/{lang}.po"));
        check_cmd(&mut compile_po);
    }
}

fn check_cmd(cmd: &mut Command) {
    let prog = cmd.get_program().to_str().unwrap().to_owned();
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                panic!("{prog} failed to run, exit: {status}")
            }
        }
        Err(error) => match error.kind() {
            ErrorKind::NotFound => panic!("{prog} not available"),
            _ => panic!("{prog} failed to run"),
        },
    }
}

fn gen_config() {
    if let Ok(contents) = toml::to_string(&Config::default()) {
        fs::write(Path::new("src/res/rrandr.toml"), contents).expect("should write default config");
    }
}
