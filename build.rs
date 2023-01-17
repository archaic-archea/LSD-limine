use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/boot.S");

    let mut command = Command::new("riscv64-elf-as");
    command.args(["-c", "src/asm/boot.s", "-o", "target/objects/boot.o"]);

    let _ = command.spawn();


    println!("cargo:rustc-link-search=target/objects/boot.o");

    println!("cargo:rustc-link-arg=conf/linker.ld");
}