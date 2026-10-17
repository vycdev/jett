//! Inspect one fixture's first native object blocker with its source function.
//! Usage: cargo run -p jett_driver --example native_probe -- FILE.jett
use jett_codegen_cranelift::{emit_host_object, symbol_name};
use jett_driver::lower_file_for_backend;
use std::path::Path;

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("usage: native_probe FILE.jett");
    let lowered = lower_file_for_backend(Path::new(&path)).expect("checked lowering");
    match emit_host_object(&lowered.mir, &lowered.interner) {
        Ok(object) => println!(
            "native object: {} bytes, {} symbols",
            object.bytes.len(),
            object.symbols.len()
        ),
        Err(error) => {
            let message = error.to_string();
            println!("{message}");
            for function in &lowered.mir.functions {
                let symbol =
                    symbol_name(&function.identity, &lowered.interner).expect("native symbol");
                if message.contains(&symbol) {
                    let declaration = &function.identity.declaration;
                    println!(
                        "source function: {}::{}",
                        declaration.namespace, declaration.name
                    );
                    break;
                }
            }
        }
    }
}
