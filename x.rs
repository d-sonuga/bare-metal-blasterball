fn main() {
    let c = first(32, 32, 32);
}

#[no_mangle]
extern "C" fn first(x: i32, y: i32, z: i32) -> i32 {
    second(32, 31, 30)
}

#[no_mangle]
extern "C" fn second(x: i32, y: i32, z: i32) -> i32 {
    432
}


