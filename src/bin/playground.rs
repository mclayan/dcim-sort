use std::path::PathBuf;
use std::panic::panic_any;

enum Element {
    Complex(String),
    Simple(String)
}

fn main() {
    let parent = Element::Complex(String::from("I have children!"));
    let single = Element::Simple(String::from("I am the last of my branch!"));

    let inp = vec!["asdf sdf", "", "never"];
    for s in inp {
        let result = match s {
            "never" => String::from("never"),
            "" => String::from("EMPTY"),
            _ => format!("_{}", s.replace(' ', "-"))
        };
        println!("result: {}", result);
    }

    let mut path = PathBuf::from("/usr/fantasy");
    path.push("child_folder");
    path.push(PathBuf::from("/usr/bin/foo.elf").as_path().file_name().unwrap());
    println!("path={}", path.to_str().unwrap());
    println!("path={}", path.to_str().unwrap());
}