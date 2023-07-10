extern crate blockchain;
extern crate console_error_panic_hook;
extern crate web_sys;

//use std::borrow::BorrowMut;
use std::{panic};
use wasm_bindgen::prelude::*;
mod storage;
mod utils;
pub mod wallet;
pub mod session;
//use crate::storage::*;
//use crate::session::*;
//use crate::utils::*;
//use blockchain::shared::Result;
use blockchain::account::*;

#[wasm_bindgen]
pub fn initialize_stuff() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    let account_identifier = AccountIdentifier{ id: "din_mamma".to_string() };
    log!("{}", account_identifier.id);
    //web_sys::console::log_1(&"Hello, world!".into());
    // ...
}
