use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

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
    let path2 = PathBuf::from("/usr/fantasy/child_folder/foo.elf");
    path.push("child_folder");
    path.push(PathBuf::from("/usr/bin/foo.elf").as_path().file_name().unwrap());
    println!("path={}", path.to_str().unwrap());
    println!("path={}", path.to_str().unwrap());

    let mut hasher = DefaultHasher::new();
    let mut hasher2 = DefaultHasher::new();
    path.hash(&mut hasher);
    let h1 = hasher.finish();
    path2.as_path().hash(&mut hasher2);
    let h2 = hasher2.finish();
    println!("h1: {}\nh2: {}", h1, h2);

}