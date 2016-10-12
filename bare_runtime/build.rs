extern crate gcc;

fn main() {
	gcc::compile_library("libos.a", &["src/os/x86_64_linux.S"]);
}
