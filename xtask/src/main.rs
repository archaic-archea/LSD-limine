use structopt::StructOpt;

#[derive(StructOpt)]
enum Command {
    Build {},
    Run {
        #[structopt(long)]
        debug: bool,
    },
}

fn build_kernel() -> anyhow::Result<()> {
    let _dir = xshell::pushd("./bare-bones");
    xshell::cmd!("cargo build --release").run()?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Command::from_args();

    match args {
        Command::Build {} => {
            build_kernel()?;
        },
        Command::Run { debug } => {
            build_kernel()?;

            let debug_log: &[&str] = match debug {
                true => &["-D", "debug.log"],
                false => &[],
            };

            #[rustfmt::skip]
            xshell::cmd!("
                qemu-system-riscv64
                    -machine virt
                    -cpu rv64
                    -smp 1
                    -m 128M
                    -bios opensbi-riscv64-generic-fw_jump.bin
                    -kernel bare-bones/target/riscv64gc-unknown-none-elf/release/bare_bones
                    -serial mon:stdio
                    -nographic
                    {debug_log...}
            ").run()?;
        }
    }

    Ok(())
}
