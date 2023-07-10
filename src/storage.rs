extern crate web_sys;

use magic_crypt::{new_magic_crypt, MagicCryptTrait, MagicCrypt256};
use std::sync::{Arc, Mutex};
//use wasm_bindgen::prelude::*;

/// Represent an entity capable of storing key-value pairs. 
pub trait Storage {

    /// Returns `None` if no value found.
    fn get(&self, key: &str) -> Option<String>;

    /// Save `value` using `key`.
	fn set(&mut self, key: &str, value: &str);

    /// Deletes an entry from the storage.
    fn delete(&mut self, key: &str);

    /// Remove all key-value pairs. Intended mainly for debugging.
    fn clear(&mut self);
}

/// A `Storage` implementation where the values are encrypted.
#[derive(std::fmt::Debug)]
pub struct KeyChain<T> where T: Storage {
    storage: Arc<Mutex<T>>,
    encryption: MagicCrypt256
}

impl<T> KeyChain<T> where T: Storage {
    
    pub fn new(storage: Arc<Mutex<T>>, password: &str) -> Self {
        Self { encryption: new_magic_crypt!(password, 256), storage: storage }
	}

    pub fn check_password(storage: &dyn Storage, account_name: &str, password: &str) -> bool {
        if let Some(encrypted_password) = storage.get(account_name) {
            let encryption: MagicCrypt256 = new_magic_crypt!(password, 256);
            if let Ok(decrypted) = encryption.decrypt_base64_to_string(&encrypted_password) {
                return decrypted == password;
            }
        }
        false
    }

    fn get_decrypted(&self, key: &str) -> blockchain::shared::Result<Option<String>> {
        let storage = self.storage.lock().unwrap();
        if let Some(encrypted_value) = storage.get(key) {
            let decrypted = &self.encryption
                .decrypt_base64_to_string(&encrypted_value)?;
                
			return Ok(Some(decrypted.to_owned()));
        }
        Ok(None)
    }
}

use std::borrow::BorrowMut;

impl<T> Storage for KeyChain<T> where T: Storage {
    
    fn get(&self, key: &str) -> Option<String> {

        if let Some(value) = self.get_decrypted(key).expect("Unable to retrieve key. Unable to decrypt.") {
			return Some(value.to_string().to_owned());
        }
        None
    }

	fn set(&mut self, key: &str, value: &str) {
        let mut unlocked = self.storage.lock().unwrap();
        let storage = unlocked.borrow_mut();

        let encrypted_value = &self.encryption.encrypt_str_to_base64(value.to_string());
        storage.set(key, encrypted_value);
    }

    fn delete(&mut self, key: &str) {
        let mut unlocked = self.storage.lock().unwrap();
        let storage = unlocked.borrow_mut();
        storage.delete(key);
    }

    fn clear(&mut self) {
        let mut unlocked = self.storage.lock().unwrap();
        let storage = unlocked.borrow_mut();
        storage.clear();   
    }
}

#[derive(std::fmt::Debug)]
pub struct DefaultStorage {
    internal_storage: Mutex<web_sys::Storage>
} 

impl DefaultStorage {

    pub fn new(internal_storage: web_sys::Storage) -> Self {
        Self { internal_storage: Mutex::new(internal_storage) }
    }
}

unsafe impl Send for DefaultStorage {}

impl Storage for DefaultStorage {

    fn get(&self, key: &str) -> Option<String> {
        self.internal_storage.lock().unwrap().get(key).expect(format!("Unable to retrieve value for key '{}' from web_sys::Storage", key).as_str())
    }

	fn set(&mut self, key: &str, value: &str) {
        self.internal_storage.lock().unwrap().set_item(key, value).expect(format!("Unable to save value for key '{}' from web_sys::Storage.", key).as_str());
    }

    fn delete(&mut self, key: &str) {
        self.internal_storage.lock().unwrap().remove_item(key).expect(format!("Unable to delete value for key '{}' from web_sys::Storage.", key).as_str());
    }

    fn clear(&mut self) {
        self.internal_storage.lock().unwrap().clear().expect("Unable to clear web_sys::Storage");
    }
}
