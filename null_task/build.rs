fn main() {
    println!("cargo:rerun-if-changed=virt.lds");
    println!("cargo:rustc-link-arg=--script=virt.lds");
}