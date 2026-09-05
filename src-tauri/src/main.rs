#[cfg(feature = "desktop")]
fn main() {
    batring_lib::run();
}

#[cfg(not(feature = "desktop"))]
fn main() {}
