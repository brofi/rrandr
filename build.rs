fn main() {
    glib_build_tools::compile_resources(
        &["src/res"],
        "src/res/rrandr.gresource.xml",
        "rrandr.gresource",
    );
    println!("cargo::rustc-env=RRANDR_COPYRIGHT_NOTICE={}", copyright_notice());
}

fn copyright_notice() -> String {
    let from = 2024;
    let to = time::OffsetDateTime::now_utc().year();
    let years = if to > from { format!("{}-{}", from, to) } else { from.to_string() };
    let authors = env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>();
    let author = authors[0].split("<").collect::<Vec<_>>()[0].trim();
    format!("Copyright (C) {} {}.", years, author)
}
