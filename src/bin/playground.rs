enum Element {
    Complex(String),
    Simple(String)
}

fn main() {
    let parent = Element::Complex(String::from("I have children!"));
    let single = Element::Simple(String::from("I am the last of my branch!"));


}