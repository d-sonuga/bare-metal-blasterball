use crate::lazy_static;
use crate::Lazy;

lazy_static! {
    static ref TRUE_OR_FALSE: bool = {
        if func(){
            true
        } else {
            false
        }
    };
}

#[test_case]
fn test_static(){
    assert_eq!(*TRUE_OR_FALSE, false);
}

fn func() -> bool {
    return false;
}