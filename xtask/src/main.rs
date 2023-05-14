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
    let _dir = xshell::pushd("./LSD");
    xshell::cmd!("cargo build --release").run()?;

    Ok(())
}

fn build_user() -> anyhow::Result<()> {
    let dir = xshell::pushd("./LSD-Userspace");
    xshell::cmd!("cargo build --release").run()?;
    std::mem::drop(dir);

    let dir = xshell::pushd("./null_task");
    xshell::cmd!("cargo build --release").run()?;
    std::mem::drop(dir);

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Command::from_args();

    match args {
        Command::Build {} => {
            build_user()?;
            build_kernel()?;
        },
        Command::Run { debug } => {
            build_user()?;
            build_kernel()?;

            let debug_log: &[&str] = match debug {
                //true => &["-d", "int,guest_errors,trace:virtio_rng_guest_not_ready,trace:virtio_rng_cpu_is_stopped,trace:virtio_rng_popped,trace:virtio_rng_pushed,trace:virtio_rng_request,trace:virtio_rng_vm_state_change,trace:virtio_set_status,trace:virtio_notify,trace:virtio_queue_notify,trace:qmp_enter_x_query_virtio_queue_element,trace:memory_region_ram_device_read,trace:memory_region_ram_device_write", "-D", "debug.log"],
                true => &["-d", "int,guest_errors", "-D", "debug.log"],
                false => &[],
            };

            xshell::cmd!("rm -rf root/boot").run()?;
            xshell::cmd!("mkdir -p root/boot").run()?;
            xshell::cmd!("cp config/spark.cfg root/boot").run()?;
            xshell::cmd!("cp LSD/target/riscv64gc-unknown-none-elf/release/lsd root/boot").run()?;

            #[rustfmt::skip]
            xshell::cmd!("
                qemu-system-riscv64
                    -machine virt
                    -cpu rv64,svpbmt=on
                    -smp 2
                    -m 8G
                    -bios opensbi-riscv64-generic-fw_jump.bin
                    -kernel config/spark-riscv-sbi-release.bin
                    -global virtio-mmio.force-legacy=false
                    -device virtio-rng-device
                    -device virtio-keyboard-device
                    -device nvme,serial=deadbeff,drive=disk1
                    -drive id=disk1,format=raw,if=none,file=fat:rw:./root
                    -serial mon:stdio
                    {debug_log...}
            ").run()?;
        }
    }

    Ok(())
}
