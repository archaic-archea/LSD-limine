qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -bios opensbi-riscv64-generic-fw_dynamic.bin \
    -m 256m \
    -global virtio-mmio.force-legacy=false \
    -s \
    -serial stdio \
    -kernel target/riscv64imac-unknown-none-elf/debug/risc-v-bare
