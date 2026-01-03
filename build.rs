use clap::CommandFactory;
use clap_mangen::Man;

#[path = "src/cli.rs"]
mod cli;

fn main() -> std::io::Result<()> {
    let out_dir =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let cmd = cli::Cli::command();

    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write(out_dir.join("waylog.1"), buffer)?;

    Ok(())
}
