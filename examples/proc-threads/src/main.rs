use std::time;
use std::thread;

fn main() {
    println!("My TID: {}", nix::unistd::gettid());

    let child = thread::spawn(move || {
        println!("My thread's TID: {}", nix::unistd::gettid());
        loop {
            thread::sleep(time::Duration::from_secs(1));
        }
    });

    child.join().unwrap();
}
